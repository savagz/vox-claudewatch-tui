use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::{Semaphore, mpsc, oneshot, watch};

use crate::config::Config;
use crate::event::AppEvent;

const MAX_HEAD: usize = 16 * 1024;
/// Un evento pesa pocos KB; el margen cubre las ráfagas que el mod reenvía
/// juntas tras una caída.
const MAX_BODY: usize = 1024 * 1024;
const READ_TIMEOUT: Duration = Duration::from_secs(10);
/// Conexiones atendidas a la vez; las que sobran se cierran sin leerlas.
const MAX_CONNECTIONS: usize = 32;

/// Consultas que el servidor le hace al estado de la app (vive en el bucle
/// principal, así que se piden por el canal y se responden por `oneshot`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Query {
    Health,
    Debug,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerStatus {
    Listening(String),
    Failed(String),
}

pub fn address(config: &Config) -> String {
    format!("{}:{}", config.server.host, config.server.port)
}

/// Bind síncrono: se usa antes del wizard de verificación para saber si el
/// puerto está libre antes de entrar al tablero.
pub fn bind(addr: &str) -> Result<std::net::TcpListener, String> {
    let listener = std::net::TcpListener::bind(addr).map_err(|e| e.to_string())?;
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    Ok(listener)
}

/// Servidor HTTP mínimo que recibe los eventos del mod `activity-beacon`:
///
/// - `POST /events` (o `/event`): un evento JSON o una lista; exige `x-token`
///   si hay token configurado.
/// - `GET /health`: lo usa el mod para saber si vuelve a haber alguien escuchando.
/// - `GET /debug`: estado por sesión y últimos eventos crudos. Exige token
///   siempre: sin token configurado no se sirve.
///
/// Guardar host o puerto desde el overlay de configuración re-abre el socket
/// en caliente; el token nuevo se aplica a la siguiente conexión.
pub fn spawn(initial: Option<std::net::TcpListener>, config_rx: watch::Receiver<Config>, tx: mpsc::Sender<AppEvent>) {
    tokio::spawn(async move {
        let mut config_rx = config_rx;
        let (mut addr, mut token) = {
            let config = config_rx.borrow();
            (address(&config), Arc::<str>::from(config.effective_token()))
        };
        let slots = Arc::new(Semaphore::new(MAX_CONNECTIONS));
        let mut listener = match initial.map(TcpListener::from_std) {
            Some(Ok(listener)) => Some(listener),
            _ => listen(&addr, &tx).await,
        };

        loop {
            let changed = match &listener {
                Some(l) => tokio::select! {
                    accepted = l.accept() => {
                        if let Ok((stream, _)) = accepted {
                            let Ok(slot) = slots.clone().try_acquire_owned() else {
                                tracing::warn!("demasiadas conexiones a la vez, se descarta una");
                                continue;
                            };
                            let (token, tx) = (token.clone(), tx.clone());
                            tokio::spawn(async move {
                                let _ = serve(stream, &token, &tx).await;
                                drop(slot);
                            });
                        }
                        continue;
                    }
                    changed = config_rx.changed() => changed,
                },
                None => config_rx.changed().await,
            };
            if changed.is_err() {
                break;
            }

            let (new_addr, new_token) = {
                let config = config_rx.borrow();
                (address(&config), Arc::<str>::from(config.effective_token()))
            };
            token = new_token;
            if new_addr != addr || listener.is_none() {
                addr = new_addr;
                // Suelta el socket viejo antes de abrir el nuevo: puede ser el mismo puerto.
                drop(listener.take());
                listener = listen(&addr, &tx).await;
            }
        }
    });
}

async fn listen(addr: &str, tx: &mpsc::Sender<AppEvent>) -> Option<TcpListener> {
    let (listener, status) = match TcpListener::bind(addr).await {
        Ok(l) => (Some(l), ServerStatus::Listening(addr.to_string())),
        Err(e) => (None, ServerStatus::Failed(format!("{addr}: {e}"))),
    };
    match &status {
        ServerStatus::Listening(a) => tracing::info!(addr = %a, "servidor escuchando"),
        ServerStatus::Failed(e) => tracing::error!(error = %e, "no se pudo abrir el servidor"),
    }
    let _ = tx.send(AppEvent::Server(status)).await;
    listener
}

#[derive(Debug, PartialEq, Eq)]
pub struct Request {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Request {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }

    /// Sin token configurado cualquiera está autorizado (salvo en `/debug`).
    pub fn authorized(&self, token: &str) -> bool {
        token.is_empty() || self.header("x-token").is_some_and(|sent| same(sent.as_bytes(), token.as_bytes()))
    }
}

/// Comparación en tiempo constante: no corta en el primer byte distinto, así
/// el tiempo de respuesta no revela cuánto del token se acertó.
fn same(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |diff, (x, y)| diff | (x ^ y)) == 0
}

#[derive(Debug, PartialEq, Eq)]
pub enum HttpError {
    Closed,
    Bad,
    TooLarge,
}

async fn serve<S: AsyncRead + AsyncWrite + Unpin>(
    mut stream: S,
    token: &str,
    tx: &mpsc::Sender<AppEvent>,
) -> std::io::Result<()> {
    let (status, body) = match tokio::time::timeout(READ_TIMEOUT, read_request(&mut stream, token)).await {
        Err(_) | Ok(Err(HttpError::Closed)) => return Ok(()),
        Ok(Err(HttpError::Bad)) => (400, r#"{"error":"http"}"#.to_string()),
        Ok(Err(HttpError::TooLarge)) => (413, r#"{"error":"size"}"#.to_string()),
        Ok(Ok(req)) => route(&req, token, tx).await,
    };
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        413 => "Payload Too Large",
        _ => "Service Unavailable",
    };
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes()).await?;
    stream.write_all(body.as_bytes()).await?;
    stream.shutdown().await
}

pub async fn route(req: &Request, token: &str, tx: &mpsc::Sender<AppEvent>) -> (u16, String) {
    let authorized = req.authorized(token);
    match (req.method.as_str(), req.path.as_str()) {
        ("GET", "/health") => ask(Query::Health, tx).await,
        // `/debug` devuelve prompts y rutas: nunca sin token.
        ("GET", "/debug") if token.is_empty() || !authorized => (401, r#"{"error":"token"}"#.to_string()),
        ("GET", "/debug") => ask(Query::Debug, tx).await,
        ("POST", "/events" | "/event") if !authorized => (401, r#"{"error":"token"}"#.to_string()),
        ("POST", "/events" | "/event") => {
            let Ok(body) = serde_json::from_slice::<Value>(&req.body) else {
                return (400, r#"{"error":"json"}"#.to_string());
            };
            let events: Vec<Value> = match body {
                Value::Array(list) => list.into_iter().filter(Value::is_object).collect(),
                obj @ Value::Object(_) => vec![obj],
                _ => Vec::new(),
            };
            if !events.is_empty() && tx.send(AppEvent::Beacon(events)).await.is_err() {
                return (503, r#"{"error":"closing"}"#.to_string());
            }
            (200, r#"{"ok":true}"#.to_string())
        }
        _ => (404, r#"{"error":"not found"}"#.to_string()),
    }
}

async fn ask(query: Query, tx: &mpsc::Sender<AppEvent>) -> (u16, String) {
    let (reply, answer) = oneshot::channel();
    if tx.send(AppEvent::Query(query, reply)).await.is_err() {
        return (503, r#"{"error":"closing"}"#.to_string());
    }
    match answer.await {
        Ok(body) => (200, body),
        Err(_) => (503, r#"{"error":"closing"}"#.to_string()),
    }
}

/// Lee una petición HTTP/1.1: cabeceras, y cuerpo por `Content-Length` o `chunked`.
/// Si la petición no trae el token, el cuerpo no se lee: `route` la rechaza
/// igual y así nadie sin token puede hacer que se reserve memoria.
pub async fn read_request<R: AsyncRead + Unpin>(r: &mut R, token: &str) -> Result<Request, HttpError> {
    let mut buf = Vec::with_capacity(4096);
    let head_end = loop {
        if let Some(pos) = find(&buf, b"\r\n\r\n") {
            break pos;
        }
        if buf.len() > MAX_HEAD {
            return Err(HttpError::TooLarge);
        }
        if fill(r, &mut buf).await? == 0 {
            return Err(if buf.is_empty() { HttpError::Closed } else { HttpError::Bad });
        }
    };

    let head = std::str::from_utf8(&buf[..head_end]).map_err(|_| HttpError::Bad)?;
    let mut lines = head.split("\r\n");
    let mut first = lines.next().ok_or(HttpError::Bad)?.split_whitespace();
    let method = first.next().ok_or(HttpError::Bad)?.to_string();
    let target = first.next().ok_or(HttpError::Bad)?;
    let path = target.split('?').next().unwrap_or(target).to_string();
    let headers: Vec<(String, String)> = lines
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .collect();

    let mut req = Request { method, path, headers, body: Vec::new() };
    if !req.authorized(token) {
        return Ok(req);
    }
    let mut rest = buf.split_off(head_end + 4);

    let chunked = req.header("transfer-encoding").is_some_and(|v| v.to_ascii_lowercase().contains("chunked"));
    if chunked {
        req.body = read_chunked(r, rest).await?;
    } else if let Some(len) = req.header("content-length") {
        let len: usize = len.parse().map_err(|_| HttpError::Bad)?;
        if len > MAX_BODY {
            return Err(HttpError::TooLarge);
        }
        while rest.len() < len {
            if fill(r, &mut rest).await? == 0 {
                return Err(HttpError::Bad);
            }
        }
        rest.truncate(len);
        req.body = rest;
    }
    Ok(req)
}

async fn read_chunked<R: AsyncRead + Unpin>(r: &mut R, mut buf: Vec<u8>) -> Result<Vec<u8>, HttpError> {
    let mut body = Vec::new();
    loop {
        let line_end = loop {
            if let Some(pos) = find(&buf, b"\r\n") {
                break pos;
            }
            if buf.len() > MAX_HEAD || fill(r, &mut buf).await? == 0 {
                return Err(HttpError::Bad);
            }
        };
        let size_text = std::str::from_utf8(&buf[..line_end]).map_err(|_| HttpError::Bad)?;
        let size_text = size_text.split(';').next().unwrap_or("").trim();
        let size = usize::from_str_radix(size_text, 16).map_err(|_| HttpError::Bad)?;
        buf.drain(..line_end + 2);
        if size == 0 {
            // Los trailers, si los hay, no interesan: se responde y se cierra.
            return Ok(body);
        }
        // `body.len() <= MAX_BODY` siempre: la resta no desborda y la suma,
        // que con un tamaño enorme sí lo haría, no hace falta.
        if size > MAX_BODY - body.len() {
            return Err(HttpError::TooLarge);
        }
        while buf.len() < size + 2 {
            if fill(r, &mut buf).await? == 0 {
                return Err(HttpError::Bad);
            }
        }
        body.extend_from_slice(&buf[..size]);
        buf.drain(..size + 2);
    }
}

async fn fill<R: AsyncRead + Unpin>(r: &mut R, buf: &mut Vec<u8>) -> Result<usize, HttpError> {
    let mut chunk = [0u8; 8192];
    let n = r.read(&mut chunk).await.map_err(|_| HttpError::Bad)?;
    buf.extend_from_slice(&chunk[..n]);
    Ok(n)
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    async fn parse(raw: &str) -> Result<Request, HttpError> {
        read_request(&mut raw.as_bytes(), "").await
    }

    #[tokio::test]
    async fn lee_content_length() {
        let req = parse("POST /events?x=1 HTTP/1.1\r\nHost: a\r\nX-Token: abc\r\nContent-Length: 5\r\n\r\nhola!extra").await.unwrap();
        assert_eq!(req.method, "POST");
        assert_eq!(req.path, "/events");
        assert_eq!(req.header("x-token"), Some("abc"));
        assert_eq!(req.body, b"hola!");
    }

    #[tokio::test]
    async fn lee_chunked() {
        let req = parse("POST /events HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n4\r\n[{}]\r\n2;ext\r\n  \r\n0\r\n\r\n").await.unwrap();
        assert_eq!(req.body, b"[{}]  ");
    }

    #[tokio::test]
    async fn rechaza_peticiones_rotas() {
        assert_eq!(parse("").await, Err(HttpError::Closed));
        assert_eq!(parse("GET / HTTP/1.1\r\n").await, Err(HttpError::Bad));
        assert_eq!(parse("POST / HTTP/1.1\r\nContent-Length: 99999999\r\n\r\n").await, Err(HttpError::TooLarge));
    }

    #[tokio::test]
    async fn chunk_gigante_no_desborda() {
        // Antes `body.len() + size` daba la vuelta y `&buf[..size]` hacía panic.
        let raw = "POST /events HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n1\r\na\r\nffffffffffffffff\r\nx";
        assert_eq!(parse(raw).await, Err(HttpError::TooLarge));
        let raw = "POST /events HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\nffffffffffffffff\r\nx";
        assert_eq!(parse(raw).await, Err(HttpError::TooLarge));
    }

    #[tokio::test]
    async fn sin_token_no_se_lee_el_cuerpo() {
        // El cuerpo anunciado nunca llega: si se intentara leer, daría `Bad`.
        let raw = "POST /events HTTP/1.1\r\nContent-Length: 1000\r\n\r\n";
        let req = read_request(&mut raw.as_bytes(), "secreto").await.unwrap();
        assert!(req.body.is_empty());
        let raw = "POST /events HTTP/1.1\r\nX-Token: secreto\r\nContent-Length: 2\r\n\r\n{}";
        assert_eq!(read_request(&mut raw.as_bytes(), "secreto").await.unwrap().body, b"{}");
    }

    fn post(body: &str, token: Option<&str>) -> Request {
        let mut headers = vec![];
        if let Some(t) = token {
            headers.push(("X-Token".to_string(), t.to_string()));
        }
        Request { method: "POST".into(), path: "/events".into(), headers, body: body.as_bytes().to_vec() }
    }

    #[tokio::test]
    async fn eventos_con_token() {
        let (tx, mut rx) = mpsc::channel(4);
        assert_eq!(route(&post("{}", None), "secreto", &tx).await.0, 401);
        assert_eq!(route(&post("{}", Some("otro")), "secreto", &tx).await.0, 401);
        assert_eq!(route(&post("no json", Some("secreto")), "secreto", &tx).await.0, 400);
        assert_eq!(route(&post(r#"[{"kind":"a"},3,{"kind":"b"}]"#, Some("secreto")), "secreto", &tx).await.0, 200);
        match rx.recv().await {
            Some(AppEvent::Beacon(events)) => assert_eq!(events.len(), 2),
            other => panic!("se esperaba Beacon, llegó {other:?}"),
        }
        // Sin token configurado se acepta cualquiera.
        assert_eq!(route(&post("{}", None), "", &tx).await.0, 200);
    }

    #[test]
    fn compara_tokens() {
        assert!(same(b"secreto", b"secreto"));
        assert!(!same(b"secreto", b"secretx"));
        assert!(!same(b"secret", b"secreto"));
        assert!(same(b"", b""));
    }

    #[tokio::test]
    async fn debug_exige_token_siempre() {
        let (tx, _rx) = mpsc::channel(4);
        let debug = |token: Option<&str>| Request {
            method: "GET".into(),
            path: "/debug".into(),
            headers: token.map(|t| vec![("X-Token".to_string(), t.to_string())]).unwrap_or_default(),
            body: vec![],
        };
        assert_eq!(route(&debug(None), "", &tx).await.0, 401);
        assert_eq!(route(&debug(Some("")), "", &tx).await.0, 401);
        assert_eq!(route(&debug(Some("otro")), "secreto", &tx).await.0, 401);
    }

    #[tokio::test]
    async fn health_responde_desde_la_app() {
        let (tx, mut rx) = mpsc::channel(4);
        tokio::spawn(async move {
            if let Some(AppEvent::Query(Query::Health, reply)) = rx.recv().await {
                let _ = reply.send(r#"{"ok":true}"#.to_string());
            }
        });
        let req = Request { method: "GET".into(), path: "/health".into(), headers: vec![], body: vec![] };
        assert_eq!(route(&req, "secreto", &tx).await, (200, r#"{"ok":true}"#.to_string()));
    }
}

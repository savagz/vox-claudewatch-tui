# VOXClaudeWatchTUI

Una interfaz de terminal (TUI) que muestra en vivo la actividad de Claude Code de otro equipo. Construida en Rust con [ratatui](https://ratatui.rs).

Es el port a Rust de `claude-watch` con el lenguaje visual de la familia VOXPBX: recibe por HTTP los eventos del mod `activity-beacon` de Claude Code (estado, herramientas, tareas, subagentes, límites y coste) y los pinta en un tablero, con una mascota pixel art que actúa lo que está haciendo Claude.

## Características

- Servidor HTTP propio (sin dependencias extra) compatible con el mod `activity-beacon`: `POST /events`, `GET /health` y `GET /debug`, con token en la cabecera `x-token`.
- Tres mascotas pixel art animadas: **Pixo** (blob coral), **Byte** (robot) y **Michi** (gato). Cada una actúa los 20 estados de Claude (pensando, leyendo, editando, ejecutando, delegando, preguntándote, plan listo, terminado, error, durmiendo...). Se cambian con `m` o desde la configuración.
- Sonidos de 8 bits distintos para los cambios de estado importantes: sesión iniciada o terminada, prompt nuevo, pregunta, plan listo, subagente, turno terminado y error. Se sintetizan al arrancar (no hay archivos de audio) y se reproducen con `pw-play`, `paplay`, `aplay` o `afplay`, lo que haya. Se silencian con `v`.
- Varias sesiones a la vez: sigue automáticamente a la más reciente o fijás una a mano.
- Límites de 5 h y 7 días, ocupación del contexto, coste por sesión, por turno y del día (por proyecto).
- Estadísticas del día persistidas en disco: turnos, tiempo activo y esperándote, tokens y caché, archivos leídos y editados, errores, herramientas más usadas y actividad por minuto.
- Registro de actividad completo y desplazable.
- Verificación del sistema: puerto, token, estadísticas, URL para el mod, reproductor de sonido.
- Interfaz en español e inglés, conmutable en caliente.
- Modo demo con una sesión simulada que recorre todos los estados.

## Requisitos

- Linux x86_64 para el binario precompilado. En macOS, compilá desde el código fuente. Windows no está soportado: compila, pero sin `/dev/urandom` no se genera el token y el servidor queda sin autenticación.
- Una terminal con truecolor para ver bien la mascota.
- Opcional, para el sonido: PipeWire (`pw-play`), PulseAudio (`paplay`) o ALSA (`aplay`). En macOS se usa `afplay`.
- En el equipo donde corre Claude Code, el mod `activity-beacon` de `claude-watch`.

## Instalación

### Script de instalación

```sh
curl -fsSL https://raw.githubusercontent.com/savagz/vox-claudewatch-tui/main/install.sh | sh
```

Opciones (se pasan con `sh -s --`): `--prefix <dir>`, `--version <tag>`, `--uninstall`, `-h`. La variable `VOXCLAUDEWATCH_VERSION` equivale a `--version`.

El script comprueba el SHA-256 del paquete, pero el `.sha256` viene del mismo release: detecta descargas corruptas, no prueba la autoría. Si eso te importa, revisá el script antes de ejecutarlo y descargá el release a mano.

### Desde el código fuente

Requiere Rust 1.88 o posterior.

```sh
cargo build --release
./target/release/voxclaudewatch-tui
```

### Línea de comandos

```
voxclaudewatch-tui [OPCIONES]

    --demo           Arranca con una sesión simulada para ver todos los estados
    --token          Muestra el token que debe usar el mod (lo genera si no hay)
    -V, --version    Muestra la versión y sale
    -h, --help       Muestra la ayuda y sale
```

`VOXCLAUDEWATCH_DEMO=1` equivale a `--demo`.

## Conectar el mod

El flujo es push: la TUI escucha en `host:puerto` y el mod `activity-beacon`, dentro de Claude Code, le envía cada evento con `POST /events`. La TUI nunca consulta a Claude. El equipo de la TUI tiene que aceptar conexiones entrantes al puerto (8765 por defecto); el de Claude Code sólo necesita llegar hasta él.

1. En el equipo de la TUI, `voxclaudewatch-tui --token` imprime el token. Si no había ninguno, la TUI lo genera y lo guarda (también lo hace al arrancar), así el servidor nunca queda abierto a toda la red. Si ya usabas `claude-watch`, se reutiliza `~/.config/claude-watch/token`. `GET /debug` exige siempre el token.
2. En el equipo de Claude Code, instalá el mod `activity-beacon` (ver abajo) y, en el bloque `env` de `~/.claude/settings.json`:
   - `CLAUDE_CODE_PLUGIN_DIRS`: incluye `~/.claude/mods/activity-beacon` (separado con `:` de otros mods)
   - `CLAUDE_WATCH_URL`: `http://<ip-del-equipo-de-la-tui>:8765` (la pestaña Verificación muestra la URL exacta)
   - `CLAUDE_WATCH_TOKEN`: el token
3. Arrancá `voxclaudewatch-tui` y reiniciá Claude Code. `/claude-watch status` confirma la conexión.

La TUI ocupa el mismo puerto que `claude-watch`: no pueden correr a la vez en el mismo equipo.

### El comando `/claude-watch`

| Comando                       | Efecto                                                        |
|-------------------------------|---------------------------------------------------------------|
| `/claude-watch` o `status`    | Destino actual y si responde                                  |
| `/claude-watch on` / `off`    | Activa o pausa el envío (se recuerda entre sesiones)          |
| `/claude-watch local`         | Envía a la TUI de este equipo (`http://127.0.0.1:8765`)       |
| `/claude-watch remote`        | Envía a `CLAUDE_WATCH_URL`                                    |
| `/claude-watch <url>`         | Envía a cualquier otra TUI, por ejemplo `http://10.0.0.5:8765` |

Cambiar de destino es en caliente: cierra la sesión en la TUI anterior, la abre en la nueva y se recuerda para las sesiones siguientes. Las sesiones ya abiertas conservan su destino hasta que corras el comando en ellas. Para usar `local` y `remote` con el mismo token, la TUI local tiene que tenerlo: guardalo en `~/.config/claude-watch/token` (permisos `0600`) o en el campo Token de la configuración (`c`).

### Prompt para crear el mod

Si el equipo de Claude Code todavía no tiene el mod, pegá esto en Claude Code:

```text
Creá un mod de Claude Code llamado activity-beacon en ~/.claude/mods/activity-beacon
(plugin con .claude-plugin/plugin.json y un módulo de hooks en hooks/register.ts
declarado en hooks/hooks.json). Su trabajo es enviar la actividad de Claude Code a
voxclaudewatch-tui por HTTP.

Configuración:
- CLAUDE_WATCH_URL (por defecto http://127.0.0.1:8765) y CLAUDE_WATCH_TOKEN, leídos
  con $.env.get en session.start.
- Destino guardado en $.store bajo "target": "local" (http://127.0.0.1:8765),
  "remote" (CLAUDE_WATCH_URL) o una URL http(s) explícita. Por defecto "remote".
- "enabled" en $.store (por defecto true).

Envío:
- Cada evento es JSON { kind, sid, project, cwd, ts, ...datos } (sid = id de sesión,
  project = último segmento de cwd, ts = Date.now()). Se encolan (máximo 500) y cada
  300 ms se envían en lote con POST <url>/events, cabeceras content-type:
  application/json y x-token: <token>.
- Si un POST falla, marcar offline y vaciar la cola; cada 60 s probar GET <url>/health
  y al volver enviar un "snapshot".
- En session.end enviar "session_end" y vaciar la cola de forma síncrona.

Eventos (kind):
- session_start y snapshot: { model, state, detail, measure, tasks }
- state: { state, detail, tool? }. Estados: idle, working, thinking (máx. 1/s),
  answering, reading, searching, editing, running, web, delegating, asking,
  planning, plan-ready, organizing, mcp, tool, done, error. Mapeo de herramientas:
  Read/NotebookRead→reading, Grep/Glob/ToolSearch/LSP→searching,
  Edit/Write/NotebookEdit→editing, Bash→running, WebFetch/WebSearch→web,
  Agent→delegating, AskUserQuestion→asking, EnterPlanMode→planning,
  ExitPlanMode→plan-ready, TaskCreate/TaskUpdate/TodoWrite→organizing,
  mcp__*→mcp, resto→tool. detail: ruta, patrón, descripción de Bash, URL o
  consulta, recortado a 120 caracteres.
- turn_start { prompt (160 car.), model }, turn_done { ms, aborted, reason, usage }
  (luego state done, idle si se abortó o error).
- tool_done { tool, detail, ok, ms, agentId }
- agent_start { agentId, type, description, model, background }, agent_turn,
  agent_tool { agentId, tool, detail }, agent_done { agentId, ms, aborted }
- tasks { tasks: [{ id, subject, status }] } a partir de TodoWrite, TaskCreate y
  TaskUpdate
- measure { measure, model }, con measure = { context: { percent, tokens, window },
  fiveHour, sevenDay: { percent, resetsAt } | null, usd }, desde session.measure
  y $.session.usage().
- session_end { reason }

Comando /claude-watch (registrado en session.start con $.command.register,
argumentHint "on | off | status | local | remote | <url>"):
- on/off: guarda enabled; al activar, envía un snapshot.
- status o vacío: prueba GET /health y responde
  "claude-watch: on, enviando a <url> [local|remote|url]." o el estado sin conexión.
- local, remote o una URL: si cambia, envía session_end { reason: "moved" } al
  destino anterior, guarda el nuevo en $.store, envía session_start con el snapshot
  y comprueba /health.

Al terminar, corré `claude plugin validate ~/.claude/mods/activity-beacon`.
```

### Prompt para activar las notificaciones

Con el mod ya creado y el token de la TUI a mano, pegá esto en Claude Code (cambiá la IP y el token):

```text
Activá el envío de actividad a voxclaudewatch-tui. En el bloque "env" de
~/.claude/settings.json, sin tocar el resto del archivo:
- agregá ~/.claude/mods/activity-beacon a CLAUDE_CODE_PLUGIN_DIRS (separado con ":"
  si ya hay otros mods),
- CLAUDE_WATCH_URL = http://<ip-del-equipo-de-la-tui>:8765
- CLAUDE_WATCH_TOKEN = <token de `voxclaudewatch-tui --token`>
Después comprobá con curl que http://<ip>:8765/health responde y decime que reinicie
Claude Code.
```

Tras reiniciar, `/claude-watch on` y `/claude-watch status`.

## Pestañas

| Tecla | Pestaña        | Contenido                                                     |
|-------|----------------|---------------------------------------------------------------|
| `1`   | Ahora          | Mascota, estado, límites, tareas, subagentes, actividad reciente |
| `2`   | Sesiones       | Sesiones vivas y detalle de la seleccionada                   |
| `3`   | Estadísticas   | Límites, coste, contadores del día, herramientas, actividad   |
| `4`   | Actividad      | Registro completo                                             |
| `5`   | Verificación   | Servidor, token, estadísticas, mod, sonido, demo              |

`Tab` y `Shift+Tab` recorren las pestañas.

## Atajos

| Tecla                 | Acción                                          |
|-----------------------|-------------------------------------------------|
| `q`, `Ctrl+C`         | Salir                                           |
| `s`                   | Fijar la siguiente sesión                       |
| `a`                   | Volver a seguir a la sesión más reciente        |
| `d`                   | Arrancar la demo                                |
| `m`                   | Cambiar de mascota                              |
| `v`                   | Activar o silenciar los sonidos                 |
| `c`, `F1`             | Configuración                                   |
| `h`                   | Ayuda                                           |
| `j`/`k`, flechas      | Navegar / desplazar                             |
| `g`/`G`, `Home`/`End` | Ir al inicio / fin                              |
| `Enter`               | Fijar la sesión seleccionada (Sesiones)         |
| `r`                   | Re-verificar (Verificación)                     |

## Sonidos

| Momento                 | Sonido                                  |
|-------------------------|-----------------------------------------|
| Sesión iniciada         | Arpegio corto ascendente                |
| Sesión terminada        | Arpegio corto descendente               |
| Prompt nuevo            | Blip                                    |
| Claude te pregunta      | Ding-dong que termina en nota alta      |
| Plan listo para aprobar | Escala ascendente abierta               |
| Subagente lanzado       | Doble boop                              |
| Turno terminado         | Fanfarria de victoria                   |
| Error                   | Zumbido descendente                     |

Un mismo sonido no se repite antes de 1,2 s, para que una ráfaga de eventos no sature. Los WAV se escriben en el directorio de caché del usuario (`~/.cache/voxclaudewatch-tui/sounds/` en Linux).

## Configuración

Funciona sin archivo de configuración. Los ajustes (host, puerto, token, idioma, mascota y sonido) se editan desde el overlay que abre `c` y se aplican al instante; cambiar host o puerto re-abre el servidor sin reiniciar. El archivo se busca en:

1. `config/voxclaudewatch.toml` relativo al directorio actual, sólo en builds de desarrollo (`cargo run`; ignorado por git). Los binarios release no lo miran.
2. `config.toml` en el directorio de config del usuario (`~/.config/voxclaudewatch-tui/` en Linux).

Ver [`config/voxclaudewatch.example.toml`](config/voxclaudewatch.example.toml).

## Datos y logs

- Estadísticas del día: `stats.json` en el directorio de datos del usuario (`~/.local/share/voxclaudewatch-tui/`), guardado cada 30 s y al salir.
- Logs diarios `voxclaudewatch-tui.log.<AAAA-MM-DD>` en el mismo directorio, nunca a la terminal. Nivel por defecto `info`; cambialo con `RUST_LOG`.

## Desarrollo

```sh
cargo test
cargo clippy --all-targets
cargo run -- --demo
```

## Estructura del proyecto

```
src/
  main.rs        Punto de entrada, wizard de arranque, bucle de eventos
  app.rs         Estado de la aplicación y manejo de teclas
  beacon/        Dominio: sesiones y eventos (mod.rs), medidas, estadísticas,
                 servidor HTTP y demo
  mascot.rs      Pixo, Byte y Michi en pixel art de medios bloques
  sound.rs       Síntesis y reproducción de los sonidos
  config.rs      Carga, guardado y overlay de configuración
  deps.rs        Chequeos de la verificación del sistema
  setup.rs       Estado de la verificación del sistema
  i18n.rs        Textos en español e inglés
  keys.rs        Mapeo de teclas a acciones
  theme.rs       Paleta y glifos del lenguaje visual
  ui/            Render: pestañas y widgets
```

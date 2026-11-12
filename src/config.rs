use color_eyre::eyre::{Result, WrapErr, eyre};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use crate::i18n::{self, Key, Lang};
use crate::mascot::Mascot;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Interfaz donde escucha el servidor de eventos.
    pub host: String,
    pub port: u16,
    /// Token que el mod envía en `x-token`. Vacío: se usa el del instalador de
    /// claude-watch (`~/.config/claude-watch/token`) si existe.
    #[serde(default)]
    pub token: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiConfig {
    /// También marca el ritmo de la animación de la mascota.
    pub tick_rate_ms: u64,
    #[serde(default)]
    pub language: Lang,
    #[serde(default)]
    pub mascot: Mascot,
    /// Sonidos en los cambios de estado importantes.
    #[serde(default = "default_sound")]
    pub sound: bool,
    /// Notificaciones de escritorio en los avisos que te esperan. Apagadas por
    /// defecto: interrumpen más que un sonido.
    #[serde(default)]
    pub notify: bool,
}

fn default_sound() -> bool {
    true
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default)]
    pub ui: UiConfig,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self { host: "0.0.0.0".to_string(), port: 8765, token: String::new() }
    }
}

impl Default for UiConfig {
    fn default() -> Self {
        Self { tick_rate_ms: 180, language: Lang::default(), mascot: Mascot::default(), sound: true, notify: false }
    }
}

impl Config {
    /// `Ok(None)` significa "no hay config en ningún lado": se usan los valores
    /// por defecto sin correr ningún asistente.
    pub fn load() -> Result<Option<Self>> {
        let Some(path) = Self::existing_path() else {
            return Ok(None);
        };
        let raw = std::fs::read_to_string(&path)
            .wrap_err_with(|| format!("no se pudo leer la config en {}", path.display()))?;
        let config: Config = toml::from_str(&raw)
            .wrap_err_with(|| format!("config inválida en {}", path.display()))?;
        Ok(Some(config))
    }

    pub fn save(&self) -> Result<PathBuf> {
        let lang = self.ui.language;
        let path = Self::write_path(lang)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).wrap_err_with(|| {
                i18n::t(lang, Key::ConfigErrCreateDirTemplate)
                    .replacen("{}", &parent.display().to_string(), 1)
            })?;
        }

        let body = toml::to_string_pretty(self).wrap_err(i18n::t(lang, Key::ConfigErrSerialize))?;
        let contents = format!("# Generado por voxclaudewatch-tui.\n\n{body}");
        write_private(&path, contents.as_bytes()).wrap_err_with(|| {
            i18n::t(lang, Key::ConfigErrWriteTemplate).replacen("{}", &path.display().to_string(), 1)
        })?;
        Ok(path)
    }

    /// Si no hay token efectivo, genera uno y guarda la config para que la TUI
    /// y el mod usen el mismo. Devuelve dónde se guardó, o `None` si ya había.
    pub fn ensure_token(&mut self) -> Result<Option<PathBuf>> {
        if !self.effective_token().is_empty() {
            return Ok(None);
        }
        self.set_new_token(Config::save).map(Some)
    }

    /// Si no se puede guardar, el token se descarta: uno que sólo vive en
    /// memoria cambiaría en cada arranque y el mod nunca acertaría.
    fn set_new_token(&mut self, save: impl FnOnce(&Config) -> Result<PathBuf>) -> Result<PathBuf> {
        self.server.token = generate_token().wrap_err("no se pudo generar el token")?;
        save(self).inspect_err(|_| self.server.token.clear())
    }

    pub fn existing_path() -> Option<PathBuf> {
        if let Some(dev) = dev_config_path().filter(|p| p.exists()) {
            return Some(dev);
        }
        let user = user_config_path()?;
        user.exists().then_some(user)
    }

    pub fn write_path(lang: Lang) -> Result<PathBuf> {
        if let Some(path) = Self::existing_path() {
            return Ok(path);
        }
        user_config_path().ok_or_else(|| eyre!(i18n::t(lang, Key::ConfigErrNoUserDir)))
    }

    /// Token con el que se validan los eventos: el de la config o, si está
    /// vacío, el del instalador de claude-watch.
    pub fn effective_token(&self) -> String {
        if !self.server.token.trim().is_empty() {
            return self.server.token.trim().to_string();
        }
        legacy_token_path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .map(|t| t.trim().to_string())
            .unwrap_or_default()
    }
}

/// `config/voxclaudewatch.toml` relativo al directorio actual, sólo en builds
/// de desarrollo: en release, arrancar desde un repo ajeno no debe cargar su
/// host ni su token, ni escribir el tuyo ahí.
fn dev_config_path() -> Option<PathBuf> {
    cfg!(debug_assertions).then(|| PathBuf::from("config/voxclaudewatch.toml"))
}

fn user_config_path() -> Option<PathBuf> {
    directories::ProjectDirs::from("", "", "voxclaudewatch-tui")
        .map(|dirs| dirs.config_dir().join("config.toml"))
}

pub fn legacy_token_path() -> Option<PathBuf> {
    directories::BaseDirs::new().map(|d| d.home_dir().join(".config/claude-watch/token"))
}

/// Escribe un archivo que guarda el token: se crea ya con permisos 0600 (nunca
/// pasa por los de la umask) y se renombra encima del anterior, que así
/// también queda en 0600 aunque antes fuera legible por otros.
fn write_private(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("toml.tmp");
    let _ = std::fs::remove_file(&tmp);
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let result = options.open(&tmp).and_then(|mut f| f.write_all(contents)).and_then(|()| std::fs::rename(&tmp, path));
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

/// Token aleatorio de 32 caracteres hexadecimales, leído de `/dev/urandom`.
fn generate_token() -> std::io::Result<String> {
    let mut bytes = [0u8; 16];
    std::fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigField {
    Host,
    Port,
    Token,
    Language,
    Mascot,
    Sound,
    Notify,
}

impl ConfigField {
    pub const ALL: [ConfigField; 7] = [
        ConfigField::Host,
        ConfigField::Port,
        ConfigField::Token,
        ConfigField::Language,
        ConfigField::Mascot,
        ConfigField::Sound,
        ConfigField::Notify,
    ];

    /// Campos que se eligen con ← → en vez de escribirse.
    pub fn is_choice(self) -> bool {
        matches!(self, ConfigField::Language | ConfigField::Mascot | ConfigField::Sound | ConfigField::Notify)
    }

    pub fn label(&self, lang: Lang) -> &'static str {
        match self {
            ConfigField::Host => i18n::t(lang, Key::ConfigFieldHostLabel),
            ConfigField::Port => i18n::t(lang, Key::ConfigFieldPortLabel),
            ConfigField::Token => i18n::t(lang, Key::ConfigFieldTokenLabel),
            ConfigField::Language => i18n::t(lang, Key::ConfigFieldLanguageLabel),
            ConfigField::Mascot => i18n::t(lang, Key::ConfigFieldMascotLabel),
            ConfigField::Sound => i18n::t(lang, Key::ConfigFieldSoundLabel),
            ConfigField::Notify => i18n::t(lang, Key::ConfigFieldNotifyLabel),
        }
    }

    pub fn hint(&self, lang: Lang) -> &'static str {
        match self {
            ConfigField::Host => i18n::t(lang, Key::ConfigFieldHostHint),
            ConfigField::Port => i18n::t(lang, Key::ConfigFieldPortHint),
            ConfigField::Token => i18n::t(lang, Key::ConfigFieldTokenHint),
            ConfigField::Language => i18n::t(lang, Key::ConfigFieldLanguageHint),
            ConfigField::Mascot => i18n::t(lang, Key::ConfigFieldMascotHint),
            ConfigField::Sound => i18n::t(lang, Key::ConfigFieldSoundHint),
            ConfigField::Notify => i18n::t(lang, Key::ConfigFieldNotifyHint),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ConfigOutcome {
    None,
    Save(Box<Config>),
    Cancel,
}

/// Overlay de configuración (`c`): dirección del servidor, token, idioma,
/// mascota y sonido.
#[derive(Debug, Clone)]
pub struct ConfigForm {
    pub focus: usize,
    pub host: String,
    pub port: String,
    pub token: String,
    pub language: Lang,
    pub mascot: Mascot,
    pub sound: bool,
    pub notify: bool,
    pub error: Option<String>,
    tick_rate_ms: u64,
}

impl ConfigForm {
    pub fn new(config: &Config) -> Self {
        Self {
            focus: 0,
            host: config.server.host.clone(),
            port: config.server.port.to_string(),
            token: config.server.token.clone(),
            language: config.ui.language,
            mascot: config.ui.mascot,
            sound: config.ui.sound,
            notify: config.ui.notify,
            error: None,
            tick_rate_ms: config.ui.tick_rate_ms,
        }
    }

    pub fn field(&self) -> ConfigField {
        ConfigField::ALL[self.focus % ConfigField::ALL.len()]
    }

    /// El token sólo se ve mientras se edita.
    pub fn display_value_of(&self, field: ConfigField) -> String {
        match field {
            ConfigField::Host => self.host.clone(),
            ConfigField::Port => self.port.clone(),
            ConfigField::Token if self.field() == ConfigField::Token => self.token.clone(),
            ConfigField::Token => "•".repeat(self.token.chars().count()),
            ConfigField::Language => self.language.label().to_string(),
            ConfigField::Mascot => self.mascot.label(self.language).to_string(),
            ConfigField::Sound => {
                i18n::t(self.language, if self.sound { Key::SoundOn } else { Key::SoundOff }).to_string()
            }
            ConfigField::Notify => {
                i18n::t(self.language, if self.notify { Key::NotifyOnShort } else { Key::NotifyOffShort }).to_string()
            }
        }
    }

    fn value_mut(&mut self, field: ConfigField) -> &mut String {
        match field {
            ConfigField::Host => &mut self.host,
            ConfigField::Port => &mut self.port,
            ConfigField::Token => &mut self.token,
            ConfigField::Language | ConfigField::Mascot | ConfigField::Sound | ConfigField::Notify => unreachable!("{field:?} no es un campo de texto"),
        }
    }

    fn accepts(field: ConfigField, c: char) -> bool {
        match field {
            ConfigField::Host => c.is_ascii_alphanumeric() || matches!(c, '.' | ':' | '-' | '[' | ']'),
            ConfigField::Port => c.is_ascii_digit(),
            ConfigField::Token => c.is_ascii_graphic(),
            ConfigField::Language | ConfigField::Mascot | ConfigField::Sound | ConfigField::Notify => false,
        }
    }

    pub fn on_key(&mut self, key: crossterm::event::KeyEvent) -> ConfigOutcome {
        use crossterm::event::KeyCode;

        self.error = None;
        match key.code {
            KeyCode::Esc => ConfigOutcome::Cancel,
            KeyCode::Up => {
                self.focus = (self.focus + ConfigField::ALL.len() - 1) % ConfigField::ALL.len();
                ConfigOutcome::None
            }
            KeyCode::Down | KeyCode::Tab => {
                self.focus = (self.focus + 1) % ConfigField::ALL.len();
                ConfigOutcome::None
            }
            KeyCode::Left | KeyCode::Right if self.field() == ConfigField::Language => {
                self.language = self.language.toggle();
                ConfigOutcome::None
            }
            KeyCode::Left | KeyCode::Right if self.field() == ConfigField::Sound => {
                self.sound = !self.sound;
                ConfigOutcome::None
            }
            KeyCode::Left | KeyCode::Right if self.field() == ConfigField::Notify => {
                self.notify = !self.notify;
                ConfigOutcome::None
            }
            KeyCode::Left if self.field() == ConfigField::Mascot => {
                self.mascot = self.mascot.prev();
                ConfigOutcome::None
            }
            KeyCode::Right if self.field() == ConfigField::Mascot => {
                self.mascot = self.mascot.next();
                ConfigOutcome::None
            }
            KeyCode::Backspace if !self.field().is_choice() => {
                let field = self.field();
                self.value_mut(field).pop();
                ConfigOutcome::None
            }
            KeyCode::Char(c) if Self::accepts(self.field(), c) => {
                let field = self.field();
                self.value_mut(field).push(c);
                ConfigOutcome::None
            }
            KeyCode::Enter => self.save(),
            _ => ConfigOutcome::None,
        }
    }

    fn save(&mut self) -> ConfigOutcome {
        let lang = self.language;
        let host = self.host.trim();
        if host.is_empty() {
            self.error = Some(
                i18n::t(lang, Key::ConfigErrInvalidTemplate).replacen("{}", ConfigField::Host.label(lang), 1),
            );
            return ConfigOutcome::None;
        }
        let port = match self.port.parse::<u16>() {
            Ok(p) if p > 0 => p,
            _ => {
                self.error = Some(
                    i18n::t(lang, Key::ConfigErrInvalidTemplate).replacen("{}", ConfigField::Port.label(lang), 1),
                );
                return ConfigOutcome::None;
            }
        };

        let config = Config {
            server: ServerConfig { host: host.to_string(), port, token: self.token.trim().to_string() },
            ui: UiConfig {
                tick_rate_ms: self.tick_rate_ms,
                language: self.language,
                mascot: self.mascot,
                sound: self.sound,
                notify: self.notify,
            },
        };
        ConfigOutcome::Save(Box::new(config))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn la_config_por_defecto_ida_y_vuelta() {
        let config = Config::default();
        let raw = toml::to_string_pretty(&config).unwrap();
        assert_eq!(toml::from_str::<Config>(&raw).unwrap(), config);
        assert_eq!(toml::from_str::<Config>("").unwrap(), config);
    }

    #[test]
    fn una_config_vieja_sin_notify_lo_deja_apagado() {
        let raw = "[ui]\ntick_rate_ms = 180\nsound = true\n";
        let config = toml::from_str::<Config>(raw).unwrap();
        assert!(config.ui.sound);
        assert!(!config.ui.notify);
    }

    #[test]
    fn el_formulario_valida_el_puerto() {
        let mut form = ConfigForm::new(&Config::default());
        form.on_key(key(KeyCode::Down));
        for _ in 0..4 {
            form.on_key(key(KeyCode::Backspace));
        }
        form.on_key(key(KeyCode::Char('x')));
        assert!(form.port.is_empty());
        assert!(matches!(form.on_key(key(KeyCode::Enter)), ConfigOutcome::None));
        assert!(form.error.is_some());
        for c in "9000".chars() {
            form.on_key(key(KeyCode::Char(c)));
        }
        match form.on_key(key(KeyCode::Enter)) {
            ConfigOutcome::Save(c) => assert_eq!(c.server.port, 9000),
            other => panic!("se esperaba Save, llegó {other:?}"),
        }
    }

    #[test]
    fn el_token_se_oculta_fuera_de_foco() {
        let mut config = Config::default();
        config.server.token = "abc".into();
        let mut form = ConfigForm::new(&config);
        assert_eq!(form.display_value_of(ConfigField::Token), "•••");
        form.focus = 2;
        assert_eq!(form.display_value_of(ConfigField::Token), "abc");
    }

    #[test]
    fn el_selector_de_mascota_cambia_con_flechas() {
        let mut form = ConfigForm::new(&Config::default());
        form.focus = 4;
        form.on_key(key(KeyCode::Right));
        assert_eq!(form.mascot, Mascot::Byte);
        assert_eq!(form.display_value_of(ConfigField::Mascot), "Byte · robot");
        form.on_key(key(KeyCode::Left));
        form.on_key(key(KeyCode::Left));
        assert_eq!(form.mascot, Mascot::Bruma);
        assert_eq!(form.display_value_of(ConfigField::Mascot), "Bruma · fantasma");
        form.language = Lang::En;
        assert_eq!(form.display_value_of(ConfigField::Mascot), "Bruma · ghost");
        match form.on_key(key(KeyCode::Enter)) {
            ConfigOutcome::Save(c) => assert_eq!(c.ui.mascot, Mascot::Bruma),
            other => panic!("se esperaba Save, llegó {other:?}"),
        }
    }

    #[test]
    fn el_token_nuevo_se_guarda_o_se_descarta() {
        let mut config = Config::default();
        let path = config.set_new_token(|c| {
            assert_eq!(c.server.token.len(), 32);
            Ok(PathBuf::from("guardado.toml"))
        });
        assert_eq!(path.unwrap(), PathBuf::from("guardado.toml"));
        assert_eq!(config.server.token.len(), 32);

        let mut config = Config::default();
        assert!(config.set_new_token(|_| Err(eyre!("disco lleno"))).is_err());
        assert!(config.server.token.is_empty());
    }

    #[test]
    fn ensure_token_respeta_el_token_existente() {
        let mut config = Config::default();
        config.server.token = "abc".into();
        assert!(config.ensure_token().unwrap().is_none());
        assert_eq!(config.server.token, "abc");
    }

    #[cfg(unix)]
    #[test]
    fn el_archivo_de_config_solo_lo_lee_el_dueno() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("voxclaudewatch-config-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        std::fs::write(&path, "viejo").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();

        write_private(&path, b"token = \"x\"").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "token = \"x\"");
        assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
        assert!(!path.with_extension("toml.tmp").exists());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn la_config_del_directorio_actual_solo_en_desarrollo() {
        assert_eq!(dev_config_path().is_some(), cfg!(debug_assertions));
    }

    #[test]
    fn genera_tokens_distintos() {
        let a = generate_token().unwrap();
        assert_eq!(a.len(), 32);
        assert_ne!(a, generate_token().unwrap());
    }
}

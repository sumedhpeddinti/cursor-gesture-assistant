use serde::{Deserialize, Serialize};
use std::{env, fs, io, path::{Path, PathBuf}};

pub mod protocol;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StartupMode {
    Manual,
    Auto,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SelectionMode {
    TextFirst,
    ScreenshotFallback,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppConfig {
    pub api_key: Option<String>,
    pub startup_mode: StartupMode,
    pub no_history: bool,
    pub helper_port: u16,
    pub ui_opacity: f32,
    pub gesture_threshold: u8,
    pub selection_mode: SelectionMode,
    pub model_name: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            api_key: None,
            startup_mode: StartupMode::Manual,
            no_history: true,
            helper_port: 48_881,
            ui_opacity: 0.88,
            gesture_threshold: 12,
            selection_mode: SelectionMode::TextFirst,
            model_name: "gemini-2.5-flash".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HelperStatus {
    pub running: bool,
    pub waiting_for_gesture: bool,
    pub last_action: Option<String>,
}

impl Default for HelperStatus {
    fn default() -> Self {
        Self {
            running: true,
            waiting_for_gesture: true,
            last_action: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum HelperCommand {
    Ping,
    GetStatus,
    UpdateConfig(AppConfig),
    SimulateGesture,
    Shutdown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum HelperReply {
    Pong,
    Status(HelperStatus),
    Ack,
    Error { message: String },
}

impl AppConfig {
    pub fn load(path: impl AsRef<Path>) -> io::Result<Self> {
        let path = path.as_ref();
        let contents = fs::read_to_string(path)?;
        serde_json::from_str(&contents).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
    }

    pub fn save(&self, path: impl AsRef<Path>) -> io::Result<()> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let contents = serde_json::to_string_pretty(self)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        fs::write(path, contents)
    }
}

pub fn default_config_path() -> PathBuf {
    let mut base = if cfg!(windows) {
        env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
    } else {
        env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| env::var_os("HOME").map(PathBuf::from).map(|home| home.join(".config")))
            .unwrap_or_else(|| PathBuf::from("."))
    };

    base.push("CursorGestureAssistant");
    base.push("config.json");
    base
}

pub fn default_data_dir() -> PathBuf {
    let mut base = if cfg!(windows) {
        env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
    } else {
        env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| env::var_os("HOME").map(PathBuf::from).map(|home| home.join(".local/share")))
            .unwrap_or_else(|| PathBuf::from("."))
    };

    base.push("CursorGestureAssistant");
    base
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_round_trip() {
        let temp_dir = std::env::temp_dir().join("cursor-core-test");
        let path = temp_dir.join("config.json");
        let config = AppConfig::default();
        config.save(&path).unwrap();
        let loaded = AppConfig::load(&path).unwrap();
        assert_eq!(loaded, config);
        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir_all(&temp_dir);
    }
}

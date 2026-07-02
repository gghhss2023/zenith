use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    pub appearance: Appearance,
    pub terminal: Terminal,
    pub ai: Ai,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Appearance {
    pub font_family: String,
    pub font_size: f32,
    pub window_opacity: f32,
    pub theme: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Terminal {
    pub scrollback_lines: usize,
    pub shell: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            appearance: Appearance::default(),
            terminal: Terminal::default(),
            ai: Ai::default(),
        }
    }
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            font_family: "Menlo".into(),
            font_size: 14.0,
            window_opacity: 1.0,
            theme: "default".into(),
        }
    }
}

impl Default for Terminal {
    fn default() -> Self {
        Self {
            scrollback_lines: 10_000,
            shell: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Ai {
    pub model: String,
}

impl Default for Ai {
    fn default() -> Self {
        Self {
            model: "sonnet".into(),
        }
    }
}

impl Config {
    pub fn load() -> Self {
        let path = Self::config_path();
        if path.exists() {
            let content = std::fs::read_to_string(&path).unwrap_or_default();
            toml::from_str(&content).unwrap_or_default()
        } else {
            Self::default()
        }
    }

    pub fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("~/.config"))
            .join("zenith")
            .join("zenith.toml")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ai_model_default_and_override() {
        let c: Config = toml::from_str("").unwrap();
        assert_eq!(c.ai.model, "sonnet");
        let c: Config = toml::from_str("[ai]\nmodel = \"opus\"").unwrap();
        assert_eq!(c.ai.model, "opus");
    }
}

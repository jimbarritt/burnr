use std::io;
use std::path::PathBuf;

use serde::Deserialize;

/// User configuration, loaded from `~/.config/burnr.toml`. Every field has a
/// default and unknown fields are ignored, so a missing or partial file is
/// fine; a file that exists but doesn't parse is a genuine user error and is
/// surfaced rather than silently replaced with defaults.
#[derive(Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct Config {
    pub idle: IdleConfig,
}

/// How the fire behaves between turns: while a burst happened within the
/// last `after_seconds`, the ambient smoulder runs at
/// `active_embers_per_second`; once the session goes quiet it dies down to a
/// gentle flicker at `embers_per_second`.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(default)]
pub struct IdleConfig {
    pub after_seconds: f32,
    pub embers_per_second: f32,
    pub active_embers_per_second: f32,
}

impl Default for IdleConfig {
    fn default() -> Self {
        Self {
            after_seconds: 10.0,
            embers_per_second: 4.0,
            active_embers_per_second: 30.0,
        }
    }
}

impl Config {
    /// Loads `~/.config/burnr.toml`, falling back to defaults if the file
    /// (or the home directory) doesn't exist.
    pub fn load() -> io::Result<Self> {
        match config_path() {
            Some(path) if path.exists() => Self::parse(&std::fs::read_to_string(&path)?),
            _ => Ok(Self::default()),
        }
    }

    fn parse(text: &str) -> io::Result<Self> {
        toml::from_str(text)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, format!("burnr.toml: {err}")))
    }
}

// Deliberately `~/.config` on every platform (not `dirs::config_dir()`,
// which is `~/Library/Application Support` on macOS) — a dotfile-style TOML
// is what users expect to hand-edit.
fn config_path() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".config").join("burnr.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_file_gives_defaults() {
        assert_eq!(Config::parse("").unwrap(), Config::default());
    }

    #[test]
    fn partial_idle_section_keeps_other_defaults() {
        let config = Config::parse("[idle]\nembers_per_second = 1.5\n").unwrap();
        assert_eq!(config.idle.embers_per_second, 1.5);
        assert_eq!(
            config.idle.after_seconds,
            IdleConfig::default().after_seconds
        );
    }

    #[test]
    fn unknown_fields_are_ignored() {
        let config = Config::parse("[idle]\nfuture_option = true\n").unwrap();
        assert_eq!(config, Config::default());
    }

    #[test]
    fn malformed_toml_is_an_error() {
        assert!(Config::parse("[idle\nnope").is_err());
    }
}

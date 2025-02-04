use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    /// File extensions to process (default: ["cs"])
    #[serde(default = "default_extensions")]
    pub extensions: Vec<String>,

    /// Paths to ignore (glob patterns)
    #[serde(default)]
    pub ignore: Vec<String>,

    /// Default input directory
    #[serde(default)]
    pub input_dir: Option<PathBuf>,

    /// Default output directory
    #[serde(default)]
    pub output_dir: Option<PathBuf>,

    /// Whether to use localization in schemas
    #[serde(default)]
    pub localized: bool,

    #[serde(default)]
    pub i18n_library: String,

    #[serde(default)]
    pub additional_imports: Vec<ImportConfig>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ImportConfig {
    pub name: String,
    pub path: String,
}

fn default_extensions() -> Vec<String> {
    vec!["cs".to_string()]
}

fn default_i18n_import() -> String {
    "vue-i18n".to_string()
}

fn default_imports() -> Vec<ImportConfig> {
    vec![]
}

impl Default for Config {
    fn default() -> Self {
        Self {
            extensions: default_extensions(),
            ignore: vec![],
            input_dir: None,
            output_dir: None,
            localized: false,
            i18n_library: default_i18n_import(),
            additional_imports: default_imports(),
        }
    }
}

impl Config {
    pub fn load() -> Result<Self, ConfigError> {
        // Look for config in current directory or parent directories
        let config_path = find_config()?;
        let content = std::fs::read_to_string(config_path)?;
        let config = toml::from_str(&content)?;
        Ok(config)
    }

    pub fn is_valid_extension(&self, path: &PathBuf) -> bool {
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| self.extensions.iter().any(|e| e == ext))
            .unwrap_or(false)
    }

    pub fn should_ignore(&self, path: &PathBuf) -> bool {
        self.ignore.iter().any(|pattern| {
            glob::Pattern::new(pattern)
                .map(|p| p.matches(&path.to_string_lossy()))
                .unwrap_or(false)
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Failed to find configuration file")]
    NotFound,

    #[error("Failed to read configuration: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Failed to parse configuration: {0}")]
    ParseError(#[from] toml::de::Error),
}

fn find_config() -> Result<PathBuf, ConfigError> {
    let mut current_dir = std::env::current_dir()?;
    loop {
        let config_path = current_dir.join("cs2ts.toml");
        if config_path.exists() {
            return Ok(config_path);
        }
        if !current_dir.pop() {
            break;
        }
    }
    Err(ConfigError::NotFound)
}

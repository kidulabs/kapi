//! Configuration loading for the kapi CLI.
//!
//! Loads server URL from `~/.kapi/config.yaml` or the `KAPI_CONFIG` env var.
//! Precedence: env var path > default path > hardcoded default.

use std::path::PathBuf;

use crate::error::CliError;

/// CLI configuration.
#[derive(Debug, Clone)]
pub struct Config {
    /// Base URL of the kapi API server.
    pub server: String,
}

impl Config {
    /// Loads configuration from the standard paths.
    ///
    /// Precedence:
    /// 1. `KAPI_CONFIG` environment variable (path to a YAML config file).
    /// 2. `~/.kapi/config.yaml` (default path).
    /// 3. Hardcoded default `http://localhost:8080`.
    pub fn load() -> Result<Self, CliError> {
        // Check KAPI_CONFIG env var for path to config file.
        let config_path = match std::env::var("KAPI_CONFIG") {
            Ok(path) => Some(PathBuf::from(path)),
            Err(_) => {
                // Default path: ~/.kapi/config.yaml
                std::env::var("HOME")
                    .ok()
                    .map(|home| PathBuf::from(home).join(".kapi").join("config.yaml"))
            }
        };

        if let Some(ref path) = config_path
            && path.exists()
        {
            let content = std::fs::read_to_string(path).map_err(|e| {
                CliError::ConfigError(format!(
                    "failed to read config file '{}': {e}",
                    path.display()
                ))
            })?;
            let config: ConfigFile = serde_yaml::from_str(&content).map_err(|e| {
                CliError::ConfigError(format!(
                    "failed to parse config file '{}': {e}",
                    path.display()
                ))
            })?;
            return Ok(Config { server: config.server });
        }

        // Hardcoded default.
        Ok(Config { server: "http://localhost:8080".to_string() })
    }
}

/// Serde-friendly struct for YAML deserialization.
#[derive(Debug, Clone, serde::Deserialize)]
struct ConfigFile {
    server: String,
}

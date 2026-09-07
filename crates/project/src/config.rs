//! Application Configuration and User Preferences.
//!
//! Handles saving and loading persistent settings according to the XDG Base Directory specification.

use crate::error::ProjectError;
use crate::schema::CanvasRatio;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tracing::{info, warn};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ThemePreference {
    System,
    Light,
    #[default]
    Dark,
}

/// Persistent user preferences for FluxCut.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppConfig {
    pub theme: ThemePreference,
    pub default_canvas: CanvasRatio,
    pub hardware_accel_enabled: bool,
    pub autosave_interval_seconds: u32,
    pub custom_cache_dir: Option<PathBuf>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            theme: ThemePreference::Dark,
            default_canvas: CanvasRatio::Landscape16x9,
            hardware_accel_enabled: true,
            autosave_interval_seconds: 60,
            custom_cache_dir: None,
        }
    }
}

impl AppConfig {
    /// Return the project directories context ("org.fluxcut.FluxCut").
    fn project_dirs() -> Option<ProjectDirs> {
        ProjectDirs::from("org", "fluxcut", "FluxCut")
    }

    /// Default config file path ($XDG_CONFIG_HOME/FluxCut/config.json).
    pub fn config_file_path() -> Result<PathBuf, ProjectError> {
        let dirs = Self::project_dirs().ok_or(ProjectError::ConfigDirectoryUnavailable)?;
        let config_dir = dirs.config_dir();
        Ok(config_dir.join("config.json"))
    }

    /// Load config from disk or return defaults if not found.
    pub fn load_or_default() -> Self {
        let path = match Self::config_file_path() {
            Ok(p) => p,
            Err(_) => return Self::default(),
        };

        if !path.exists() {
            return Self::default();
        }

        match fs::read_to_string(&path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_else(|e| {
                warn!(
                    "Corrupt config at {}: {}. Using defaults.",
                    path.display(),
                    e
                );
                Self::default()
            }),
            Err(e) => {
                warn!(
                    "Unable to read config at {}: {}. Using defaults.",
                    path.display(),
                    e
                );
                Self::default()
            }
        }
    }

    /// Save configuration to user's XDG config directory.
    pub fn save(&self) -> Result<(), ProjectError> {
        let path = Self::config_file_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| ProjectError::IoError {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        let content = serde_json::to_string_pretty(self)?;
        fs::write(&path, content).map_err(|source| ProjectError::IoError { path, source })?;

        info!("Application configuration saved successfully");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_config_defaults() {
        let cfg = AppConfig::default();
        assert_eq!(cfg.theme, ThemePreference::Dark);
        assert!(cfg.hardware_accel_enabled);
        assert_eq!(cfg.autosave_interval_seconds, 60);
    }

    #[test]
    fn test_app_config_serialization() {
        let cfg = AppConfig::default();
        let json = serde_json::to_string(&cfg).expect("Failed serialization");
        let deserialized: AppConfig = serde_json::from_str(&json).expect("Failed deserialization");
        assert_eq!(cfg, deserialized);
    }
}

//! FluxCut Diagnostics and Logging Subsystem.
//!
//! Provides structured logging via `tracing`, environment diagnostics,
//! and performance telemetry collection.

use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use thiserror::Error;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[derive(Error, Debug)]
pub enum DiagnosticsError {
    #[error("Failed to initialize logging subscriber: {0}")]
    LoggingInitError(String),
    #[error("I/O error during diagnostics collection: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
}

/// Initialize tracing logging for FluxCut.
///
/// Respects the `RUST_LOG` environment variable if present.
/// If not present, falls back to `default_directive` or `info,fluxcut=debug`.
pub fn init_logging(default_directive: Option<&str>) -> Result<(), DiagnosticsError> {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(default_directive.unwrap_or("info,fluxcut=debug")));

    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer().with_target(true))
        .try_init()
        .map_err(|e| DiagnosticsError::LoggingInitError(e.to_string()))?;

    tracing::info!("FluxCut diagnostics & logging initialized successfully");
    Ok(())
}

/// Snapshot of the host operating system, environment, and graphics stack context.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiagnosticReport {
    pub os_name: String,
    pub kernel_version: String,
    pub wayland_display: Option<String>,
    pub x11_display: Option<String>,
    pub session_type: Option<String>,
    pub desktop_session: Option<String>,
    pub rustc_version: String,
    pub application_version: String,
}

impl DiagnosticReport {
    /// Collect current environment and platform diagnostics.
    pub fn collect() -> Self {
        let os_name = fs::read_to_string("/etc/os-release")
            .ok()
            .and_then(|content| {
                content
                    .lines()
                    .find(|line| line.starts_with("PRETTY_NAME="))
                    .map(|line| {
                        line.trim_start_matches("PRETTY_NAME=")
                            .trim_matches('"')
                            .to_string()
                    })
            })
            .unwrap_or_else(|| env::consts::OS.to_string());

        let kernel_version = fs::read_to_string("/proc/version")
            .ok()
            .and_then(|v| {
                v.split_whitespace()
                    .take(3)
                    .collect::<Vec<_>>()
                    .join(" ")
                    .into()
            })
            .unwrap_or_else(|| "Unknown Kernel".to_string());

        let wayland_display = env::var("WAYLAND_DISPLAY").ok();
        let x11_display = env::var("DISPLAY").ok();
        let session_type = env::var("XDG_SESSION_TYPE").ok();
        let desktop_session = env::var("XDG_CURRENT_DESKTOP").ok();

        Self {
            os_name,
            kernel_version,
            wayland_display,
            x11_display,
            session_type,
            desktop_session,
            rustc_version: env!("CARGO_PKG_VERSION").to_string(),
            application_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    /// Whether this system has an active Wayland display session.
    pub fn is_wayland_active(&self) -> bool {
        self.wayland_display.is_some()
            || self
                .session_type
                .as_deref()
                .map(|s| s.eq_ignore_ascii_case("wayland"))
                .unwrap_or(false)
    }

    /// Render diagnostic summary as human-readable Markdown.
    pub fn format_markdown(&self) -> String {
        format!(
            "# FluxCut Diagnostic Report\n\
             - **OS:** {}\n\
             - **Kernel:** {}\n\
             - **Wayland Display:** {}\n\
             - **X11 Display:** {}\n\
             - **Session Type:** {}\n\
             - **Desktop Environment:** {}\n\
             - **App Version:** {}\n",
            self.os_name,
            self.kernel_version,
            self.wayland_display.as_deref().unwrap_or("None"),
            self.x11_display.as_deref().unwrap_or("None"),
            self.session_type.as_deref().unwrap_or("Unknown"),
            self.desktop_session.as_deref().unwrap_or("Unknown"),
            self.application_version,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diagnostic_report_collection() {
        let report = DiagnosticReport::collect();
        assert!(!report.os_name.is_empty());
        assert!(!report.application_version.is_empty());
        let md = report.format_markdown();
        assert!(md.contains("FluxCut Diagnostic Report"));
    }

    #[test]
    fn test_diagnostic_serialization() {
        let report = DiagnosticReport::collect();
        let json = serde_json::to_string(&report).expect("Failed to serialize report");
        let deserialized: DiagnosticReport =
            serde_json::from_str(&json).expect("Failed to deserialize report");
        assert_eq!(report, deserialized);
    }
}

//! FluxCut Hardware and GPU Acceleration Probing Subsystem.
//!
//! Queries the operating system, display server, DRM/DRI subsystems,
//! Vulkan ICD manifests, and VA-API driver directories to establish
//! safe hardware acceleration and presentation capabilities at runtime.

use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;
use tracing::{debug, info};

#[derive(Error, Debug)]
pub enum HardwareError {
    #[error("Failed to query hardware devices: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Hardware capability detection failed: {0}")]
    ProbeError(String),
}

/// Detected graphics and multimedia capabilities of the host system.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GpuCapabilities {
    /// Whether a Wayland compositor is active and accessible.
    pub wayland_available: bool,
    /// Whether DRM render nodes or graphics cards are present in /dev/dri.
    pub dri_available: bool,
    /// List of detected DRM device paths (e.g., /dev/dri/renderD128, /dev/dri/card0).
    pub dri_nodes: Vec<String>,
    /// Whether Vulkan ICD manifests are installed on the system.
    pub vulkan_icd_available: bool,
    /// List of detected Vulkan ICD manifest filenames.
    pub vulkan_icd_files: Vec<String>,
    /// Whether VA-API driver libraries were detected.
    pub vaapi_available: bool,
    /// List of detected VA-API driver shared libraries.
    pub vaapi_drivers: Vec<String>,
    /// Whether DMA-BUF buffer sharing is likely supported by the hardware/display stack.
    pub dmabuf_available: bool,
}

impl GpuCapabilities {
    /// Probe the current system for available GPU, Wayland, and media hardware features.
    pub fn probe() -> Self {
        let wayland_available = Self::detect_wayland();
        let dri_nodes = Self::detect_dri_nodes();
        let dri_available = !dri_nodes.is_empty();
        let vulkan_icd_files = Self::detect_vulkan_icds();
        let vulkan_icd_available = !vulkan_icd_files.is_empty();
        let vaapi_drivers = Self::detect_vaapi_drivers();
        let vaapi_available = !vaapi_drivers.is_empty() && dri_available;

        // DMA-BUF is generally usable on Linux if DRM DRI render nodes exist and
        // either Wayland is active or an accelerated driver is available.
        let dmabuf_available =
            dri_available && (wayland_available || vulkan_icd_available || vaapi_available);

        let caps = Self {
            wayland_available,
            dri_available,
            dri_nodes,
            vulkan_icd_available,
            vulkan_icd_files,
            vaapi_available,
            vaapi_drivers,
            dmabuf_available,
        };

        info!(
            wayland = caps.wayland_available,
            vulkan = caps.vulkan_icd_available,
            vaapi = caps.vaapi_available,
            dmabuf = caps.dmabuf_available,
            "Probed hardware capabilities"
        );

        caps
    }

    /// Check if Wayland display server is active.
    fn detect_wayland() -> bool {
        if env::var("WAYLAND_DISPLAY").is_ok() {
            return true;
        }
        if let Ok(session) = env::var("XDG_SESSION_TYPE") {
            if session.eq_ignore_ascii_case("wayland") {
                return true;
            }
        }
        false
    }

    /// Detect DRI devices in /dev/dri.
    fn detect_dri_nodes() -> Vec<String> {
        let dri_path = Path::new("/dev/dri");
        let mut nodes = Vec::new();

        if let Ok(entries) = fs::read_dir(dri_path) {
            for entry in entries.flatten() {
                let file_name = entry.file_name().to_string_lossy().to_string();
                if file_name.starts_with("renderD") || file_name.starts_with("card") {
                    nodes.push(entry.path().to_string_lossy().to_string());
                }
            }
        }

        nodes.sort();
        debug!(count = nodes.len(), "Discovered DRI nodes");
        nodes
    }

    /// Detect installed Vulkan ICD manifests.
    fn detect_vulkan_icds() -> Vec<String> {
        let search_dirs = [
            "/usr/share/vulkan/icd.d",
            "/etc/vulkan/icd.d",
            "/usr/local/share/vulkan/icd.d",
        ];

        let mut icds = Vec::new();
        for dir in search_dirs {
            let path = Path::new(dir);
            if let Ok(entries) = fs::read_dir(path) {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.ends_with(".json") {
                        icds.push(name);
                    }
                }
            }
        }

        icds.sort();
        icds.dedup();
        debug!(count = icds.len(), "Discovered Vulkan ICD manifests");
        icds
    }

    /// Detect installed VA-API driver libraries.
    fn detect_vaapi_drivers() -> Vec<String> {
        let candidate_dirs = [
            "/usr/lib/x86_64-linux-gnu/dri",
            "/usr/lib64/dri",
            "/usr/lib/dri",
            "/usr/local/lib/dri",
        ];

        let mut drivers = Vec::new();
        for dir in candidate_dirs {
            let path = PathBuf::from(dir);
            if let Ok(entries) = fs::read_dir(&path) {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.ends_with("_drv_video.so") || name.ends_with("vdpau_drv_video.so") {
                        drivers.push(name);
                    }
                }
            }
        }

        drivers.sort();
        drivers.dedup();
        debug!(count = drivers.len(), "Discovered VA-API driver libraries");
        drivers
    }

    /// Generate human-readable diagnostics for the Capabilities dialog and CLI.
    pub fn summary_markdown(&self) -> String {
        format!(
            "### Hardware & GPU Capabilities\n\
             - **Wayland Display:** {}\n\
             - **DRI Subsystem:** {} ({} nodes found)\n\
             - **Vulkan ICD:** {} ({})\n\
             - **VA-API Hardware Acceleration:** {} ({})\n\
             - **DMA-BUF Presentation Path:** {}\n",
            if self.wayland_available {
                "Active"
            } else {
                "Inactive / X11 Fallback"
            },
            if self.dri_available {
                "Available"
            } else {
                "Not detected"
            },
            self.dri_nodes.len(),
            if self.vulkan_icd_available {
                "Supported"
            } else {
                "Not found"
            },
            if self.vulkan_icd_files.is_empty() {
                "none".to_string()
            } else {
                self.vulkan_icd_files.join(", ")
            },
            if self.vaapi_available {
                "Available"
            } else {
                "Not detected / Software fallback"
            },
            if self.vaapi_drivers.is_empty() {
                "none".to_string()
            } else {
                self.vaapi_drivers.join(", ")
            },
            if self.dmabuf_available {
                "Supported (Zero-copy eligible)"
            } else {
                "Disabled / CPU fallback"
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hardware_probe_does_not_panic() {
        let caps = GpuCapabilities::probe();
        let summary = caps.summary_markdown();
        assert!(summary.contains("Hardware & GPU Capabilities"));
    }

    #[test]
    fn test_hardware_capabilities_serialization() {
        let caps = GpuCapabilities::probe();
        let json = serde_json::to_string(&caps).expect("Failed to serialize GpuCapabilities");
        let deserialized: GpuCapabilities =
            serde_json::from_str(&json).expect("Failed to deserialize GpuCapabilities");
        assert_eq!(caps, deserialized);
    }
}

//! Background export task controller with mode dispatch (fast-path vs transcode) and QA verification.

use crate::error::ExportError;
use crate::remux::{can_fast_path_remux, fast_path_remux};
use crate::transcode::{ExportProgress, TranscodeEngine};
use crate::validator::{verify_exported_file, ExportValidationReport};
use crossbeam_channel::{bounded, Receiver, Sender};
use fluxcut_project::Project;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use tracing::info;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportMode {
    FastPathRemux,
    Transcode,
}

impl ExportMode {
    pub fn label(self) -> &'static str {
        match self {
            ExportMode::FastPathRemux => "Fast-Path Stream Copy (Lossless Remux)",
            ExportMode::Transcode => "H.264 Video Transcode",
        }
    }
}

#[derive(Debug, Clone)]
pub enum ExportEvent {
    Started {
        mode: ExportMode,
    },
    Progress(ExportProgress),
    Finished {
        mode: ExportMode,
        report: ExportValidationReport,
    },
    Error(String),
    Cancelled,
}

pub struct ExportController {
    cancel_flag: Arc<AtomicBool>,
    event_rx: Receiver<ExportEvent>,
    worker_handle: Option<thread::JoinHandle<()>>,
}

impl ExportController {
    /// Spawn a background export worker for the given project.
    pub fn start(project: Project, output_path: PathBuf) -> Self {
        let (event_tx, event_rx) = bounded::<ExportEvent>(64);
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let flag_clone = Arc::clone(&cancel_flag);

        let worker_handle = thread::Builder::new()
            .name("fluxcut-export-worker".to_string())
            .spawn(move || {
                Self::run_export(project, output_path, event_tx, flag_clone);
            })
            .expect("Failed to spawn export worker thread");

        Self {
            cancel_flag,
            event_rx,
            worker_handle: Some(worker_handle),
        }
    }

    pub fn cancel(&self) {
        self.cancel_flag.store(true, Ordering::Relaxed);
    }

    pub fn try_recv(&self) -> Option<ExportEvent> {
        self.event_rx.try_recv().ok()
    }

    fn run_export(
        project: Project,
        output_path: PathBuf,
        event_tx: Sender<ExportEvent>,
        cancel_flag: Arc<AtomicBool>,
    ) {
        let (w, h) = (project.settings.width, project.settings.height);
        let min_dur = 0.05;

        // 1. Evaluate Fast-Path Remux eligibility
        if let Some((input_path, in_sec, out_sec)) = can_fast_path_remux(&project) {
            info!("Project qualifies for fast-path remux");
            let _ = event_tx.send(ExportEvent::Started {
                mode: ExportMode::FastPathRemux,
            });

            match fast_path_remux(&input_path, &output_path, in_sec, out_sec) {
                Ok(()) => match verify_exported_file(&output_path, Some(w), Some(h), min_dur) {
                    Ok(report) => {
                        let _ = event_tx.send(ExportEvent::Finished {
                            mode: ExportMode::FastPathRemux,
                            report,
                        });
                    }
                    Err(e) => {
                        let _ = event_tx.send(ExportEvent::Error(format!(
                            "Post-export validation failed: {}",
                            e
                        )));
                    }
                },
                Err(e) => {
                    let _ = event_tx.send(ExportEvent::Error(format!("Remux failed: {}", e)));
                }
            }
            return;
        }

        // 2. Transcode Export Pipeline
        let _ = event_tx.send(ExportEvent::Started {
            mode: ExportMode::Transcode,
        });

        let (prog_tx, prog_rx) = bounded::<ExportProgress>(32);
        let event_tx_clone = event_tx.clone();

        // Progress forwarder
        let forwarder = thread::spawn(move || {
            while let Ok(prog) = prog_rx.recv() {
                let _ = event_tx_clone.send(ExportEvent::Progress(prog));
            }
        });

        match TranscodeEngine::transcode(&project, &output_path, Some(prog_tx), cancel_flag) {
            Ok(()) => {
                let _ = forwarder.join();
                match verify_exported_file(&output_path, Some(w), Some(h), min_dur) {
                    Ok(report) => {
                        let _ = event_tx.send(ExportEvent::Finished {
                            mode: ExportMode::Transcode,
                            report,
                        });
                    }
                    Err(e) => {
                        let _ = event_tx.send(ExportEvent::Error(format!(
                            "Post-export validation failed: {}",
                            e
                        )));
                    }
                }
            }
            Err(ExportError::Cancelled) => {
                let _ = event_tx.send(ExportEvent::Cancelled);
            }
            Err(e) => {
                let _ = event_tx.send(ExportEvent::Error(format!("Transcode failed: {}", e)));
            }
        }
    }
}

impl Drop for ExportController {
    fn drop(&mut self) {
        self.cancel_flag.store(true, Ordering::Relaxed);
        if let Some(handle) = self.worker_handle.take() {
            let _ = handle.join();
        }
    }
}

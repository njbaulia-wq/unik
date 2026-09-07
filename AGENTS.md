# FluxCut — Agent & Contributor Instructions (AGENTS.md)

Welcome to the **FluxCut** codebase. This document is the primary developer manual and architectural contract for all AI coding agents and human contributors. Every change made to this repository must conform to the rules, invariants, and guidelines set forth here and in `PRD.md`.

---

## 1. Core Architectural Commandments

When working on FluxCut, treat the following principles as absolute non-negotiables:

1. **P1 — Editing is Metadata First:**  
   Trimming, cutting, splitting, moving, muting, volume adjustments, and transformations modify semantic project state only. They must never mutate or render source media files on disk during editing.
2. **P2 — Preview is Disposable:**  
   Preview textures, cached frames, and downscaled buffers are ephemeral. The correctness and fidelity of the final project must never depend on preview renders.
3. **P3 — Export is a Compiled Execution Plan:**  
   Export translates project state into an explicit Directed Acyclic Graph (`EditorGraph`), validates it, and executes it in dedicated background workers.
4. **P4 — Fast Path First:**  
   When cuts or trims fall cleanly on safe boundaries without requiring re-encoding, filters, or resizing, utilize stream-copy remuxing.
5. **P5 — Zero Main-Thread Blocking:**  
   The GTK main event loop is exclusively reserved for UI event dispatch, user input, and scene graph coordination. Probing, decoding, thumbnail extraction, waveform generation, and export must run on background worker threads.
6. **P6 — Capability-Driven Behavior & Graceful Fallback:**  
   Detect GPU capabilities (Wayland, Vulkan, VA-API, DMA-BUF) at runtime. Always provide a reliable CPU/software fallback path.

---

## 2. Strictly Prohibited Anti-Patterns

Any code submission containing any of the following will be rejected:

* ❌ **NO Electron / WebViews:** Never attempt to replace native media components with an embedded web browser or HTML5 video tag.
* ❌ **NO Shell-Out Playback:** Never run `ffmpeg` CLI sub-processes in a loop as the video preview engine. Direct library integration (`libavcodec`, `libavformat`, `libswscale`) is required.
* ❌ **NO Widget-per-Clip Timeline:** Never instantiate a separate `GtkWidget` for each timeline clip. Dense timelines must be rendered via custom snapshot drawing (`GskRenderNode`) with viewport virtualization.
* ❌ **NO Whole-File In-Memory Loading:** Never buffer entire high-resolution media files in RAM. Use streaming packet queues and bounded LRU frame caches.
* ❌ **NO Unchecked FFI Leaks:** Never expose raw FFmpeg C pointers (`AVFormatContext*`, `AVCodecContext*`, `AVFrame*`) directly to UI or timeline crates. Isolate all FFI in safe RAII abstractions inside `fluxcut-ffmpeg-core`.
* ❌ **NO Floating-Point Timelines:** Never store timeline timestamps solely as `f32` or `f64`. Always use `TimeRational` (`num / den`) for frame and sample precision.

---

## 3. Workspace Layout & Crate Boundaries

```text
crates/
├── app/               # fluxcut-app: Application bootstrap, CLI argument handling, AdwApplication lifecycle
├── ui/                # fluxcut-ui: Libadwaita/GTK4 windows, preferences, capabilities dialogs
├── timeline/          # fluxcut-timeline: (Phase 3) Custom Gsk snapshot timeline widget & interaction
├── project/           # fluxcut-project: Project schema v1, TimeRational math, AppConfig persistence, Undo/Redo
├── media/             # fluxcut-media: (Phase 2) Asset representation, metadata probing models
├── ffmpeg-core/       # fluxcut-ffmpeg-core: (Phase 2) Safe RAII abstractions over libavformat, libavcodec, libswscale
├── decode/            # fluxcut-decode: (Phase 4) Frame decoding worker loop, coalesced seeking
├── render/            # fluxcut-render: (Phase 4) Presentation bridge (GdkDmabufTextureBuilder / MemoryTexture)
├── filters/           # fluxcut-filters: (Phase 7) EditorGraph AST compiler -> libavfilter DAG
├── audio/             # fluxcut-audio: (Phase 5) Waveform downsampler, cpal audio playback monitor
├── export/            # fluxcut-export: (Phase 7) Background export pipeline, fast-path remuxer
├── cache/             # fluxcut-cache: (Phase 2) XDG-compliant disk LRU cache for thumbnails & waveforms
├── hardware/          # fluxcut-hardware: Runtime GPU, Wayland, Vulkan, VA-API, and DMA-BUF detection
└── diagnostics/       # fluxcut-diagnostics: Structured tracing subscriber, system reports, telemetry
```

### Dependency Rules:
* `fluxcut-project` has **zero UI dependencies** (pure data and serialization logic).
* `fluxcut-ui` depends on `fluxcut-project` and `fluxcut-hardware`, never raw FFmpeg bindings.
* Inter-thread communication must use thread-safe channels (`crossbeam-channel`, `std::sync::mpsc`, or `tokio::sync`).

---

## 4. Quality Gates & Development Commands

Before considering any task or phase complete, execute the following validation cycle and ensure **zero warnings and zero test failures**:

### 1. Build and Compilation Check
```bash
cargo check --workspace --all-targets
cargo build --workspace
```

### 2. Full Test Suite Execution
```bash
cargo test --workspace -- --nocapture
```

### 3. Clippy Lints (Strict Mode)
```bash
cargo clippy --workspace --all-targets -- -D warnings
```

### 4. Code Formatting
```bash
cargo fmt --all -- --check
```

---

## 5. Milestone Progression

Development proceeds strictly in sequential vertical slices:

* **Phase 0 — Foundation & Capability Proof (COMPLETED):**
  - Workspace structure, GTK4 (4.14+) / libadwaita (1.5+) integration.
  - Wayland / Vulkan / VA-API / DMA-BUF capability detection.
  - Project schema v1, `TimeRational`, and persistent `AppConfig`.
  - Structured logging via `tracing`.
  - CLI flags: `--diagnostics`, `--version`, `--help`.
* **Phase 1 — Native Shell & Layout:** HeaderBar actions, async `GtkFileDialog`, empty status page, toast notifications.
* **Phase 2 — Media Ingestion:** Asynchronous probing, safe `fluxcut-ffmpeg-core`, lazy thumbnail extraction, waveform envelopes.
* **Phase 3 — Timeline Engine:** Virtualized `GskRenderNode` timeline widget, split/trim/delete operations, snapping logic.
* **Phase 4 — Playback Engine:** Playback worker thread, coalesced seeking, `GdkDmabufTextureBuilder` presentation with `GdkMemoryTexture` fallback.
* **Phase 5 — Audio Subsystem:** `cpal` monitor stream, per-clip audio mute, track volume sliders, audio mixing.
* **Phase 6 — Image Clips & Transforms:** Still image import, overlay composition, 9:16 crop rect editor, template presets.
* **Phase 7 — Export Engine:** `EditorGraph` filter compiler, fast-path stream copy, background export with progress bar and post-export verification.
* **Phase 8 — Hardware Acceleration Tuning:** VA-API and Vulkan Video pipeline optimization, proxy generation.
* **Phase 9 — Packaging:** Flatpak manifest (`org.fluxcut.FluxCut.yaml`), desktop integration, QA acceptance matrix.

---

## 6. PRD Acceptance Scenarios

Every major feature must test against one or more of the 4 end-to-end user scenarios defined in `PRD.md`:
1. **Scenario A (Social Vertical Video):** 9:16 crop of 16:9 clip, split & delete gap, original audio muted, BGM added at 70%, PNG watermark overlay, export to 1080x1920 MP4.
2. **Scenario B (Fast Cut Remux):** In/out trim with stream-copy export without re-encoding.
3. **Scenario C (Audio Replacement & Fade):** Video original audio muted, external audio with fade-out exported cleanly.
4. **Scenario D (Still Image Template):** Image clip placed with custom duration, template applied, and exported.

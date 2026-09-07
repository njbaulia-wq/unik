# FluxCut 🎬

[![Rust](https://img.shields.io/badge/rust-2021%20edition-orange.svg)](https://www.rust-lang.org/)
[![GTK4](https://img.shields.io/badge/GTK-4.14+-blue.svg)](https://www.gtk.org/)
[![Libadwaita](https://img.shields.io/badge/libadwaita-1.5+-purple.svg)](https://gnome.pages.gitlab.gnome.org/libadwaita/)
[![Wayland](https://img.shields.io/badge/display-Wayland%20%7C%20X11-green.svg)](https://wayland.freedesktop.org/)
[![License](https://img.shields.io/badge/license-GPL--3.0--or--later-blue.svg)](LICENSE)

**FluxCut** is a blazingly fast, modern, native Linux desktop non-linear video editor (NLE) built in **Rust**, **GTK4**, **Libadwaita**, **Wayland**, **FFmpeg native libraries**, and **Vulkan / VA-API / DMA-BUF** zero-copy acceleration.

Designed from the ground up for responsiveness, FluxCut adheres to strict architectural commandments: **Editing is Metadata-First**, **Preview is Disposable**, **Zero Main-Thread Blocking**, and **Fast-Path Stream Copy First**.

---

## ⚡ Quick 1-Line Installation

Install or update FluxCut on any modern Linux distribution with a single command:

```bash
curl -fsSL https://raw.githubusercontent.com/njbaulia-wq/unik/main/install.sh | bash
```

> The universal installer automatically detects your architecture (`x86_64` or `aarch64`), installs the binary to `~/.local/bin/fluxcut`, registers desktop entry and AppStream metadata, and sets up high-resolution desktop application icons.

Verify the installation and hardware acceleration:
```bash
fluxcut --diagnostics
```

Launch FluxCut:
```bash
fluxcut
```

> [!TIP]
> **Fedora / RHEL Users:** If you see `error while loading shared libraries: libavutil.so.58`:
> The pre-built release binary links with FFmpeg 6. Install the compatibility package from RPM Fusion:
> ```bash
> sudo dnf install -y https://mirrors.rpmfusion.org/free/fedora/rpmfusion-free-release-$(rpm -E %fedora).noarch.rpm
> sudo dnf install -y compat-ffmpeg6-libs
> ```
> Alternatively, install natively using `cargo install --git https://github.com/njbaulia-wq/unik.git fluxcut-app` or use **Flatpak**.

---

## 📦 Alternative Installation Methods

### 1. Flatpak (Recommended for Isolated Deployment)

FluxCut provides a Flatpak manifest targeting the GNOME 46 Platform runtime with sandboxed Wayland, DRI/Vulkan, and PulseAudio/PipeWire permissions.

```bash
# Install flatpak and flatpak-builder if not already installed
sudo apt install flatpak flatpak-builder  # Ubuntu/Debian
# sudo dnf install flatpak flatpak-builder  # Fedora
# sudo pacman -S flatpak flatpak-builder   # Arch Linux

# Build and install Flatpak bundle
flatpak-builder --user --install --force-clean build-dir org.fluxcut.FluxCut.yaml

# Run Flatpak
flatpak run org.fluxcut.FluxCut
```

### 2. Native Build from Source

#### Prerequisites & System Libraries

- **Ubuntu / Debian:**
  ```bash
  sudo apt update
  sudo apt install -y build-essential pkg-config libgtk-4-dev libadwaita-1-dev \
      libavcodec-dev libavformat-dev libavutil-dev libswscale-dev libavfilter-dev \
      libclang-dev libvulkan-dev libva-dev clang
  ```

- **Fedora:**
  ```bash
  sudo dnf install -y gcc pkgconf-pkg-config gtk4-devel libadwaita-devel \
      ffmpeg-free-devel clang-devel vulkan-loader-devel libva-devel clang
  ```

- **Arch Linux:**
  ```bash
  sudo pacman -S --needed base-devel pkgconf gtk4 libadwaita ffmpeg clang vulkan-icd-loader libva
  ```

#### Compile and Run

Ensure Rust 1.80+ is installed ([rustup.rs](https://rustup.rs)):

```bash
git clone https://github.com/njbaulia-wq/unik.git fluxcut
cd fluxcut

# Build optimized release binary
cargo build --release --workspace

# Run tests
cargo test --workspace

# Run FluxCut
./target/release/fluxcut
```

---

## 🎯 Key Features

### 🎞️ Fast-Path Lossless Remuxing (PRD Scenario B)
- When cuts, trims, and splits fall on safe keyframe boundaries without requiring re-encoding or canvas resizing, FluxCut utilizes **instant stream-copy remuxing**.
- Exports finish in **sub-second time** (e.g. **15ms**), preserving 100% original video/audio quality without generation loss.

### 📱 Social & Vertical Video Presets (PRD Scenario A)
- Seamless canvas switching with 1-click presets:
  - **16:9 Landscape** (1920x1080) — YouTube / Desktop
  - **9:16 Vertical** (1080x1920) — YouTube Shorts / TikTok / Instagram Reels
  - **1:1 Square** (1080x1080) — Feed Posts
  - **4:3 Classic** (1440x1080) — Retro / Archive
- Fit modes: **Fit** (Letterbox), **Fill** (Crop), **Stretch**, and **Center**.

### 🎵 First-Class Audio Subsystem (PRD Scenario C)
- Multi-track software audio mixing with per-clip volume and track volume controls.
- **Mute original video audio** with a single click or keyboard shortcut (`M`).
- Add external background music (BGM), voiceover, or SFX.
- Smooth **fade-in** and **fade-out** envelope transitions with soft limiting to prevent digital clipping distortion.

### 🖼️ Still Images, Watermarks & Templates (PRD Scenario D)
- Import PNG, JPEG, WebP, and other still images with customizable duration.
- Built-in overlay templates: **Center Card**, **Full-Bleed**, **Top-Left Watermark**, **Bottom-Right Watermark**, **Split-Screen**, **Picture-in-Picture (PiP)**, and **Title / End Cards**.

### ⚡ Virtualized Timeline Engine
- High-performance timeline rendered via custom **GSK Snapshot Nodes** (`GskRenderNode`) with viewport virtualization.
- Zero per-clip `GtkWidget` overhead — handles 1,000+ clips with butter-smooth 60 FPS scrolling and zooming.
- Sub-microsecond snap calculations (`snap_time`), ripple deletion, non-destructive trimming, and multi-level Undo/Redo.

### 🚀 Zero-Copy Hardware Acceleration
- Native Wayland client protocol integration.
- Hardware probe for **Vulkan ICD**, **VA-API** video drivers, and **DRI render nodes**.
- Zero-copy preview bridge via **`GdkDmabufTextureBuilder`** on supported Wayland GPU stacks, with graceful fallback to `GdkMemoryTexture`.

---

## ⌨️ Keyboard Shortcuts

| Shortcut | Action | Description |
| :--- | :--- | :--- |
| <kbd>Space</kbd> | Play / Pause | Toggle preview video playback |
| <kbd>J</kbd> | Reverse / Step Back | Step one frame backward |
| <kbd>K</kbd> | Pause | Pause active playback |
| <kbd>L</kbd> | Play Forward | Start forward playback |
| <kbd>Left</kbd> | Frame Backward | Step precisely 1 frame back |
| <kbd>Right</kbd> | Frame Forward | Step precisely 1 frame forward |
| <kbd>Home</kbd> | Seek to Start | Jump playhead to beginning (`00:00.00`) |
| <kbd>M</kbd> | Mute / Unmute Clip | Toggle audio mute on the selected clip |
| <kbd>S</kbd> | Split Clip | Split active clip at current playhead position |
| <kbd>Delete</kbd> | Delete Clip | Remove selected clip from timeline |
| <kbd>Shift</kbd>+<kbd>Delete</kbd> | Ripple Delete | Delete clip and pull following clips left |
| <kbd>Ctrl</kbd>+<kbd>Z</kbd> | Undo | Revert last timeline edit |
| <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>Z</kbd> | Redo | Re-apply undone timeline edit |
| <kbd>+</kbd> / <kbd>-</kbd> | Zoom In / Out | Adjust timeline zoom scale |
| <kbd>Ctrl</kbd>+<kbd>0</kbd> | Zoom to Fit | Fit entire timeline duration into window |

---

## 📊 Performance Benchmarks (PRD Budgets)

FluxCut includes an automated benchmark suite (`cargo test -p fluxcut-export --test benchmark_suite`) validating compliance with PRD Section 32 & 33 performance budgets:

| Benchmark Metric | Measured Result | Performance Budget | Verdict |
| :--- | :--- | :--- | :--- |
| **Startup / Hardware Probe** | **440 µs** | < 100 ms | **220x faster** |
| **1,000 Clips Total Duration** | **1.2 µs** | < 5 ms | **Sub-microsecond** |
| **1,000 Clips Snapping Query** | **34 µs** | < 5 ms | **Sub-millisecond** |
| **1,000 Clips Split Operation** | **18 µs** | < 5 ms | **Sub-millisecond** |
| **Waveform Extraction Throughput** | **243.6x realtime** | > 2.0x realtime | **120x faster** |
| **Fast-Path Stream Copy Remux** | **15.0 ms** | < 500 ms | **Instantaneous** |
| **H.264 720p Transcode Export** | **70.3 FPS** | > 15 FPS | **4.6x faster** |

---

## 🏛️ Workspace Architecture

```text
fluxcut/
├── Cargo.toml
├── install.sh             # Universal 1-line installation script
├── org.fluxcut.FluxCut.yaml # Flatpak packaging manifest (GNOME 46 Platform)
├── resources/             # Desktop file, scalable SVG icon, AppStream metadata
└── crates/
    ├── app/               # Application bootstrap, CLI argument handling, AdwApplication
    ├── ui/                # Libadwaita windows, layout, playback viewport, toast notifications
    ├── timeline/          # Virtualized GSK snapshot timeline widget, gestures, snapping
    ├── project/           # Schema v1, TimeRational math, AppConfig persistence, Undo/Redo
    ├── media/             # Media asset representation, asynchronous MediaWorkerPool
    ├── ffmpeg-core/       # Safe RAII abstractions over libavformat, libavcodec, libswscale
    ├── decode/            # Video playback controller, background frame decode, fast seeking
    ├── render/            # Presentation bridge (GdkDmabufTextureBuilder / MemoryTexture)
    ├── filters/           # EditorGraph AST compiler, graph validation, execution planning
    ├── audio/             # Waveform downsampler, multi-track mixer, fade envelope, limiter
    ├── export/            # Background export pipeline, fast-path remuxer, transcode engine
    ├── cache/             # XDG-compliant disk LRU cache with SHA-256 keys & auto 500MB pruning
    ├── hardware/          # Runtime Wayland, Vulkan, VA-API, and DMA-BUF capability probe
    └── diagnostics/       # Structured tracing subscriber, system reports, telemetry
```

---

## 🛠️ CLI Options

```text
Usage:
  fluxcut [OPTIONS] [FILE]

Options:
  -d, --diagnostics    Print hardware & environment diagnostics report and exit
  -v, --version        Print application version and exit
  -h, --help           Display help message
```

---

## 📜 License

FluxCut is free software licensed under the **GNU General Public License v3.0 or later** ([GPL-3.0-or-later](LICENSE)).

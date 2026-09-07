# FLUXCUT — Product Requirements Document (PRD)

**Project:** FluxCut  
**Type:** Native Linux desktop video editor  
**Document:** Product Requirements Document  
**Status:** Ready for AI-agent implementation planning  
**Target platform:** Linux, Wayland-first  
**Primary UI stack:** GTK4 + libadwaita  
**Primary language:** Rust  
**Media engine:** FFmpeg 9.x libraries  
**Graphics path:** GSK/Vulkan where supported, with robust fallback paths  
**Document version:** 1.0  
**Last researched:** 2026-09-07

---

## 0. Important instruction to the AI coding agent

Treat this PRD as the source of product intent and acceptance criteria, but **do not implement the whole application in one pass**.

First:
1. inspect the development environment and installed Linux multimedia stack;
2. verify GTK4/libadwaita, Wayland, Vulkan, FFmpeg 9.x development headers/libraries, VA-API and available GPU capabilities;
3. propose the smallest vertical slice that proves the architecture;
4. implement in incremental milestones;
5. run builds/tests after every milestone;
6. never replace a difficult native-media component with Electron, WebView, or a browser-based editor;
7. never block the GTK main thread with decode, waveform generation, thumbnail generation, proxy generation, or export;
8. preserve a clean separation between UI, project state, media analysis, playback, rendering, and export.

The agent must favor **correctness, responsiveness, native Linux integration, measurable performance, and maintainability** over adding many effects quickly.

Do not claim zero-copy or hardware acceleration unless it is detected and actually used. Provide capability detection and CPU/software fallback.

---

# 1. Product summary

## 1.1 Product name

**FluxCut**

## 1.2 One-line definition

A lightweight, native Linux video editor optimized for the common workflow:

> import → cut/trim → arrange → add image/audio → mute/select audio → crop/ratio → preview → export.

## 1.3 Product thesis

Most users do not need a huge professional NLE for quick video work. They need an editor that opens quickly, seeks quickly, stays responsive, makes simple edits without intermediate rendering, and exports predictable files.

FluxCut therefore prioritizes:

- instant-feeling editing;
- non-destructive timeline operations;
- lazy media analysis;
- GPU-aware preview;
- hardware decode/encode when available;
- fast lossless/stream-copy export when technically safe;
- a focused modern Linux UI;
- social-media-oriented output presets;
- strong keyboard workflow;
- low memory and CPU overhead when idle.

---

# 2. Research-backed technical direction

## 2.1 FFmpeg target

Target **FFmpeg 9.x**. FFmpeg 9.0.1 is the latest stable release at the time of this PRD, released 2026-08-12, with libavutil 61, libavcodec 63, libavformat 63, libavfilter 12, libswscale 10 and libswresample 7.

Use FFmpeg libraries directly for the media engine where practical rather than making the `ffmpeg` CLI process the application architecture.

Recommended internal libraries:

- `libavformat` — demux/input and mux/output;
- `libavcodec` — decoding and encoding;
- `libavfilter` — media filter graphs;
- `libswscale` — software colorspace/scaling fallback;
- `libswresample` — audio sample-rate/format conversion;
- `libavutil` — timestamps, pixel/sample formats, buffers, rational timebases, hardware contexts.

A CLI adapter may exist for diagnostics and troubleshooting, but must not be the core playback/editing architecture.

## 2.2 Rust FFmpeg binding strategy

Evaluate `ffmpeg-next` 9.x / `ffmpeg-sys-next` 9.x first because a current FFmpeg 9 compatible Rust wrapper exists. If the wrapper does not expose a required API, isolate the missing functionality in a small internal FFI module using bindgen/manual bindings instead of contaminating the rest of the codebase with unsafe code.

All unsafe FFmpeg interop must be isolated behind safe Rust abstractions where feasible.

## 2.3 GTK4 / libadwaita

Use GTK4 as the UI toolkit and libadwaita for GNOME-native visual language.

Use GTK4's asynchronous file dialog APIs for import/export dialogs.

Use a custom timeline widget rather than constructing thousands of GTK child widgets. GTK4's rendering model is scene-graph based through GSK, and render-node caching/partial redraw behavior makes custom drawing suitable for a dense timeline.

## 2.4 Wayland

Wayland is the primary target and must be tested as a first-class environment. The application may retain X11 compatibility as a fallback, but Wayland behavior must never be treated as an afterthought.

Use GDK Wayland APIs only behind a small platform abstraction when genuinely needed.

## 2.5 Vulkan / DMA-BUF

Use Vulkan/GSK when supported for UI rendering and investigate GPU-frame presentation paths using DMA-BUF where the platform/driver/GTK stack supports them.

Never hard-code the assumption that every GPU can consume every DMA-BUF format/modifier. Detect supported formats and modifiers at runtime.

The media path should support three levels:

1. hardware/GPU-native path;
2. GPU processing with controlled CPU copies when needed;
3. software CPU fallback.

## 2.6 Hardware acceleration

Capability detection should identify, where available:

- VA-API;
- Intel hardware decode/encode;
- AMD hardware decode/encode;
- NVIDIA hardware decode/encode;
- Vulkan capabilities.

The editor should select the safest appropriate path rather than blindly enabling a hardware flag.

A hardware path that causes extra GPU↔CPU copies or unstable playback must be allowed to fall back to software.

---

# 3. Target users

## Persona A — Quick creator

Needs to cut screen recordings, gameplay, phone videos and short-form content quickly.

Primary needs:
- trim;
- split;
- delete gaps;
- add music;
- mute original audio;
- crop to 9:16 / 1:1 / 16:9;
- export 720p/1080p;
- fast startup.

## Persona B — Linux developer / power user

Values native UI, keyboard shortcuts, low resource use, predictable CLI/media behavior, and an open project format.

## Persona C — Portfolio / content workflow user

Wants a polished editor for YouTube Shorts, Reels, TikTok, presentations, tutorials and simple promotional videos without learning a professional NLE.

---

# 4. Product goals

## 4.1 Primary goals

1. Build a genuinely native Linux editor.
2. Make basic editing faster and simpler than a full NLE.
3. Keep the UI responsive during all media-heavy operations.
4. Avoid unnecessary intermediate rendering.
5. Use hardware acceleration when it genuinely improves throughput.
6. Support common social-video output ratios and resolutions.
7. Make audio/image insertion and muting first-class features.
8. Produce deterministic, testable exports.

## 4.2 Secondary goals

- strong keyboard-first workflow;
- reusable presets/templates;
- good accessibility and GNOME integration;
- project recovery/autosave;
- meaningful performance telemetry for debugging.

---

# 5. Non-goals for v1

Do NOT turn v1 into a DaVinci Resolve/Kdenlive competitor.

Explicitly out of v1 unless architecture allows them without destabilizing core editing:

- full color-grading suite;
- node-based compositor;
- advanced 3D effects;
- motion tracking;
- multicamera editing;
- collaborative cloud editing;
- AI video generation;
- browser-based editor;
- stock-media marketplace;
- built-in social posting APIs;
- complex VFX pipeline.

These can be future roadmap items.

---

# 6. Product principles

### P1 — Editing is metadata first

Dragging, trimming, cutting, muting, changing volume, changing clip placement, and most transforms must primarily modify project/timeline state rather than create new media files.

### P2 — Preview is disposable

Preview resources may be cached, but project correctness must never depend on preview renders.

### P3 — Export is a compiled execution plan

Convert project state into an explicit media graph / export plan, validate it, then execute it in workers.

### P4 — Fast path first

For operations that can be completed by remuxing or stream copy without violating the requested output, prefer the fast path.

### P5 — No main-thread blocking

The GTK main loop is for UI state, input, rendering coordination, and lightweight events only.

### P6 — Capability-driven behavior

Detect codec, pixel format, hardware, display backend, Vulkan support, DMA-BUF formats, and encoder availability before selecting optimized paths.

### P7 — Graceful fallback

Every acceleration layer must have a software fallback.

---

# 7. Core user experience

## 7.1 First launch

The user sees a clean empty project with:

- large `Import Media` action;
- optional drag-and-drop target;
- `New Project` state;
- compact recent projects list if available;
- output preset shortcut.

No heavy media indexing is required before the user starts working.

## 7.2 Import flow

1. User opens media dialog or drags files in.
2. App immediately registers media in the project.
3. Lightweight metadata analysis begins asynchronously.
4. Thumbnail/waveform tasks are scheduled lazily.
5. UI displays media item immediately with `Analyzing…` when metadata is pending.
6. Any failure is shown inline without crashing the project.

GTK4's asynchronous `GtkFileDialog` should be used for file selection.

## 7.3 Timeline flow

The basic workflow must be:

1. import;
2. drag clip to timeline;
3. split/trim;
4. move/reorder;
5. mute audio or detach audio if desired;
6. add music/audio;
7. add images;
8. crop or change canvas ratio;
9. preview;
10. export.

---

# 8. Main UI specification

## 8.1 Main window

Suggested layout:

```text
┌───────────────────────────────────────────────────────────────┐
│ FluxCut   New Open Save Undo Redo        Preview   Export    │
├─────────────────┬─────────────────────────────────────────────┤
│ Media Library   │                                             │
│                 │                    Preview                  │
│ + Import        │                                             │
│ Search          │                                             │
│                 │                                             │
│ video.mp4       │                                             │
│ music.mp3       │                                             │
│ image.png       │                                             │
├─────────────────┴─────────────────────────────────────────────┤
│ Timeline controls / zoom / snapping / duration                │
├───────────────────────────────────────────────────────────────┤
│ V2   ┌──────── Image ──────────┐                              │
│ V1   └──── Video ──────┬───────┴──────────┐                   │
│ A2       ┌──── Music ──┴─────────────────┐                   │
│ A1       └──── Original audio ───────────┘                    │
│                 ▲ playhead                                     │
└───────────────────────────────────────────────────────────────┘
```

The actual UI must follow GNOME HIG/libadwaita conventions rather than copying the ASCII mockup literally.

## 8.2 Visual style

- modern dark-first media workspace;
- optional system/light theme compatibility;
- compact toolbar;
- large preview;
- clear selected clip state;
- strong timeline hierarchy;
- minimal decorative UI;
- use symbolic icons where appropriate;
- avoid excessive gradients and glossy effects.

Dark-by-default is acceptable for a video-heavy app while respecting system appearance preferences.

---

# 9. Media library requirements

## FR-MEDIA-001 Import files

Support multiple file selection and drag-and-drop.

Acceptance criteria:
- selecting multiple media files registers all valid items;
- invalid/unreadable files are reported individually;
- UI never freezes while analysis occurs.

## FR-MEDIA-002 Supported input classes

At minimum, the app should rely on FFmpeg capabilities for common:

- MP4/MOV;
- MKV;
- WebM;
- AVI;
- MPEG/TS;
- common image files: PNG, JPEG, WebP, TIFF where FFmpeg or the image loader path supports them;
- common audio files: MP3, WAV, AAC/M4A, FLAC, OGG/Opus.

The implementation should query the installed FFmpeg build rather than assuming every distribution exposes every codec.

## FR-MEDIA-003 Metadata

Store/read:

- duration;
- width/height;
- display aspect ratio;
- pixel aspect ratio when present;
- frame rate;
- time base;
- video codec;
- audio codec;
- sample rate;
- channels;
- color information where available;
- stream count;
- rotation/orientation metadata.

## FR-MEDIA-004 Thumbnail generation

Thumbnails must be lazy and cached.

Requirements:
- generate only visible/near-visible frames;
- avoid full-file thumbnail extraction on import;
- cache by stable content/media identity plus time position;
- cache must be invalidatable.

## FR-MEDIA-005 Waveform generation

Generate a compact waveform representation asynchronously.

Requirements:
- downsample audio into peak/RMS bins;
- cache on disk;
- draw only visible bins;
- avoid recomputing waveform every UI frame.

---

# 10. Timeline requirements

## FR-TL-001 Tracks

Support:
- multiple video tracks;
- multiple audio tracks;
- image clips on video tracks;
- optional text/subtitle track architecture reserved for future expansion.

## FR-TL-002 Clip operations

Required:
- add;
- move;
- trim left;
- trim right;
- split at playhead;
- delete;
- duplicate;
- replace source;
- ripple delete;
- gap insertion/removal;
- snapping on/off;
- clip selection;
- multi-selection.

## FR-TL-003 Time precision

Never store timeline time only as floating-point seconds.

Use a rational/integer timestamp representation internally, e.g. timeline ticks or a rational timebase.

The UI may display decimal seconds/timecode but internal editing must preserve frame/sample precision.

## FR-TL-004 Timeline rendering

Use a custom high-performance timeline surface/widget.

Do not instantiate one GTK widget per clip.

The timeline renderer must:
- virtualize offscreen content;
- render only visible tracks/clips;
- render thumbnails only where enough horizontal space exists;
- reuse cached render data;
- avoid per-frame heap allocations where possible.

## FR-TL-005 Zoom

Support:
- keyboard zoom;
- mouse/wheel zoom;
- fit timeline to content;
- fit selected clips;
- minimum and maximum zoom levels.

## FR-TL-006 Playhead

Required:
- click-to-seek;
- drag-to-seek;
- frame stepping;
- snapping behavior;
- visible timecode.

---

# 11. Video editing requirements

## FR-VIDEO-001 Trim

Non-destructive in/out trimming per clip.

## FR-VIDEO-002 Split

Split clips at the playhead without re-encoding during editing.

## FR-VIDEO-003 Crop

Interactive crop rectangle plus presets:

- freeform;
- original;
- square;
- 16:9;
- 9:16;
- 4:3;
- 3:4;
- 1:1.

Crop must be represented as project state and compiled to FFmpeg filtering during preview/export.

## FR-VIDEO-004 Transform

Support:
- position X/Y;
- scale;
- rotation;
- horizontal flip;
- vertical flip;
- opacity.

## FR-VIDEO-005 Speed

Support:
- 0.25x;
- 0.5x;
- 0.75x;
- 1x;
- 1.25x;
- 1.5x;
- 2x;
- custom multiplier.

Audio policy must be explicit: when changing speed, preserve pitch where the selected processing path can safely do so; otherwise show that pitch may change.

## FR-VIDEO-006 Freeze frame

Reserve an implementation path for a freeze frame based on a chosen video frame.

## FR-VIDEO-007 Orientation

Respect source rotation metadata and provide user-visible rotate controls.

---

# 12. Audio requirements

This is a first-class part of v1.

## FR-AUDIO-001 Add audio

The user can import audio files and place them onto audio tracks.

Acceptance criteria:
- audio can be moved independently;
- audio can be trimmed/split;
- audio can overlap video;
- timeline displays waveform;
- export includes audio according to track state.

## FR-AUDIO-002 Original video audio mute

Every video clip containing audio must provide a clear mute control.

Examples:
- clip speaker icon;
- right-click `Mute clip audio`;
- keyboard/action command where appropriate.

Mute must affect the audio route in the project graph without modifying the source file.

At export, muted audio is either omitted from the mix or fed at silence, depending on the compiled media graph. Do not create unnecessary intermediate muted files.

## FR-AUDIO-003 Mute entire audio track

Each audio track has:
- mute;
- solo;
- volume;
- lock.

## FR-AUDIO-004 Volume

Per-clip volume and per-track volume.

UI values:
- percentage;
- decibel mode in advanced controls.

## FR-AUDIO-005 Fade

Support simple fade in/fade out per audio clip.

## FR-AUDIO-006 Audio mixing

Use an FFmpeg filter graph for mixing multiple audio sources. The implementation may use `amix` where appropriate and must handle duration differences deterministically.

## FR-AUDIO-007 Audio sample normalization

The engine must be able to resample/convert incompatible source audio formats into the project/export format.

## FR-AUDIO-008 Music over original video

Common workflow:

```text
Video original audio ── volume 0.3 ─┐
                                   ├─ mix → output
Music -----------------------------┘
```

The UI should make this achievable without forcing the user to understand a filter graph.

## FR-AUDIO-009 Audio-only export

Reserve an engine path for future audio-only rendering, but it is not a required v1 UI feature.

---

# 13. Image requirements

## FR-IMAGE-001 Add image

Allow PNG/JPEG/WebP and other supported still images to be imported to a video track.

## FR-IMAGE-002 Image as clip

An image clip must have:
- duration;
- position;
- scale;
- rotation;
- opacity;
- crop;
- optional fade.

## FR-IMAGE-003 Image overlays

Allow an image to sit above video without replacing the base video.

Use cases:
- logo;
- watermark;
- screenshot;
- intro card;
- end card;
- meme/image insert;
- picture-in-picture style layer.

## FR-IMAGE-004 Image templates

Provide built-in lightweight templates:

1. Center image card
2. Full-bleed image
3. Top-left watermark
4. Bottom-right watermark
5. Split-screen image/video
6. Picture-in-picture
7. Intro title-card background image
8. End-card image

Templates are project presets, not pre-rendered media.

## FR-IMAGE-005 Image duration

Dragging an image to the timeline uses a configurable default duration, with fast trim handles afterward.

---

# 14. Ratio / canvas / output frame requirements

This is a key feature.

## FR-CANVAS-001 Project canvas

Project has a defined output canvas:

- width;
- height;
- pixel aspect ratio;
- frame rate;
- background behavior.

## FR-CANVAS-002 Aspect ratio presets

Built-in presets:

### Landscape
- 16:9 — 1920x1080
- 16:9 — 1280x720
- 16:9 — 3840x2160

### Vertical
- 9:16 — 1080x1920
- 9:16 — 720x1280

### Square
- 1:1 — 1080x1080
- 1:1 — 720x720

### Classic
- 4:3 — 1440x1080
- 4:3 — 1024x768

### Portrait classic
- 3:4 — 1080x1440

Add a `Custom` option.

## FR-CANVAS-003 Change ratio without destroying clips

Changing project ratio must not alter source files or permanently crop clips.

Each clip is evaluated against the new canvas using a selected fit mode.

Fit modes:
- Fill / crop;
- Fit / letterbox;
- Stretch (advanced; warning recommended);
- Smart center;
- user-positioned crop.

## FR-CANVAS-004 Background

When using `Fit`, allow:
- solid background;
- blurred background derived from the same video (future optional);
- custom image background (v1 optional).

## FR-CANVAS-005 Output resolution selector

Export dialog must clearly expose:
- source/native;
- 2160p/4K where valid;
- 1440p;
- 1080p;
- 720p;
- 480p;
- custom dimensions.

If the chosen dimensions conflict with the selected ratio, the UI must explain and normalize them rather than silently generating an unintended aspect ratio.

---

# 15. Export requirements

## FR-EXPORT-001 Export dialog

Show a simple top-level preset selector plus an advanced section.

Example preset groups:

- YouTube 1080p
- YouTube Shorts 1080x1920
- Instagram/Reels 1080x1920
- Square 1080x1080
- Web 720p
- Original/source profile
- Custom

## FR-EXPORT-002 Codec presets

At minimum expose profiles for commonly available:

- H.264/AVC;
- H.265/HEVC when available;
- AV1 when available;
- VP9/WebM where supported;
- AAC audio;
- Opus for WebM or supported containers.

Actual choices must be capability-driven.

## FR-EXPORT-003 Container presets

At minimum:
- MP4;
- WebM;
- MKV.

## FR-EXPORT-004 Resolution

Show width/height and a human-readable profile name.

## FR-EXPORT-005 FPS

Choices:
- source;
- 24;
- 25;
- 30;
- 50;
- 60;
- custom where valid.

Changing frame rate must be explicit and must not silently introduce judder without the project/export logic accounting for it.

## FR-EXPORT-006 Quality

Simple quality presets:

- Fast
- Balanced
- High quality
- Small file
- Custom

Map these to codec-specific settings internally.

## FR-EXPORT-007 Hardware encoder

If a capable hardware encoder is detected, show:

`Hardware acceleration: Available`

Otherwise:

`Software encoding`

Do not present a hardware option that cannot actually be used for the selected codec/profile.

## FR-EXPORT-008 Progress

Show:
- percent;
- current stage;
- elapsed time;
- estimated remaining time where reliable;
- encode FPS;
- output size if available;
- cancel button.

## FR-EXPORT-009 Cancel

Export cancellation must:
- stop worker processing safely;
- close media contexts;
- remove or mark incomplete temporary output;
- leave the project usable.

## FR-EXPORT-010 Output verification

After export, verify at minimum:
- output file exists;
- file is readable;
- expected duration is within tolerance;
- expected video dimensions are correct;
- selected audio presence/absence is correct;
- file size is nonzero.

Optionally perform a lightweight post-export FFmpeg probe.

---

# 16. Fast-path export strategy

This is one of FluxCut's main differentiators.

## 16.1 Stream copy path

When the requested operation can be fulfilled without changing encoded media content, attempt stream copy/remux rather than re-encoding.

Typical candidates:
- simple trim/remux where codec/container constraints allow it;
- stream selection;
- metadata/stream disposition changes;
- audio removal where remuxing is sufficient.

The UI must label fast-path operations accurately, e.g.:

`Fast export — no re-encode`

Do not claim frame-exact lossless cutting where codec/keyframe constraints make that impossible.

## 16.2 Re-encode path

Required when:
- ratio/crop changes;
- scaling;
- image overlays;
- transitions;
- speed changes;
- audio mixing;
- codec change;
- frame-rate conversion;
- text/subtitle burn-in;
- filters are applied.

Compile all compatible operations into as few filter/encode stages as possible.

---

# 17. Playback architecture

## 17.1 Requirements

Playback must support:
- play/pause;
- seek;
- frame stepping;
- shuttle J/K/L-style controls if feasible;
- playback speed;
- audio monitoring;
- mute monitoring;
- preview quality selector when needed.

## 17.2 Playback state

Central playback state must include:

- current timeline position;
- playback state;
- target frame rate/timebase;
- selected preview quality;
- active sequence;
- loop range;
- audio monitoring state.

## 17.3 Seeking

Seeking must be cancellable and coalesced.

If the user rapidly drags the playhead:
- do not decode every intermediate request fully;
- cancel or supersede stale seek requests;
- prioritize the newest position;
- show the closest available frame immediately when possible.

---

# 18. Performance architecture

## 18.1 Worker pools

Use dedicated asynchronous workers/queues for:

- media probing;
- frame decoding;
- thumbnails;
- waveform generation;
- proxy generation;
- preview rendering;
- export.

Do not create unbounded threads per file.

## 18.2 Cache hierarchy

### L1 — RAM
Recently used decoded frames and timeline resources.

### L2 — GPU
Hardware textures/surfaces when available.

### L3 — SSD cache
Thumbnails, waveform envelopes, proxies and optional preview segments.

Cache paths should follow XDG cache conventions.

## 18.3 Proxy mode

Proxy generation is conditional, not automatic for every project.

Trigger recommendation when:
- codec is difficult to decode in real time;
- resolution/frame rate is high;
- hardware decoder is absent/inadequate;
- preview frame rate falls below target for a sustained period.

Proxy settings:
- low resolution;
- edit-friendly codec;
- predictable frame rate;
- cacheable identity;
- one-click disable/remove.

## 18.4 Frame-copy discipline

Avoid unnecessary copies between:

```text
GPU → CPU → GPU
```

Where the platform permits, maintain hardware surfaces through compatible stages.

However, do not compromise stability merely to preserve a theoretical zero-copy path.

---

# 19. GPU / Vulkan architecture

The product must have a hardware capability layer:

```text
GpuCapabilities {
  vulkan_available
  dmabuf_available
  supported_dmabuf_formats
  vaapi_available
  hw_decode_codecs
  hw_encode_codecs
}
```

## 19.1 Rendering

GTK4/GSK handles application UI rendering.

The preview pipeline is separate from ordinary UI widgets.

Preferred conceptual path:

```text
FFmpeg decode
      ↓
Hardware/GPU frame
      ↓
compatible GPU presentation path
      ↓
preview surface
```

Fallback:

```text
FFmpeg software decode
      ↓
CPU frame
      ↓
texture upload
      ↓
preview surface
```

## 19.2 Filters

Map high-level editor operations to FFmpeg filter graphs.

Examples include:
- crop;
- scale;
- overlay;
- blend;
- set presentation timestamps;
- volume;
- audio mix;
- audio resampling;
- frame rate conversion.

GPU-friendly/Vulkan filters should be considered when they reduce expensive CPU/GPU transfers and are available in the installed FFmpeg build.

---

# 20. Filter graph compiler

Create an internal representation independent of FFmpeg string syntax.

Example:

```text
EditorGraph
  VideoTrack
    Clip A
      Trim
      Crop
      Scale
      Transform
    Image B
      Scale
      Overlay
  AudioTrack
    Original Audio
      Mute/Volume
    Music
      Volume
  Mixer
  Output
```

Compiler responsibilities:

1. validate graph;
2. resolve timelines into source time ranges;
3. insert necessary timestamp transforms;
4. insert scale/crop operations;
5. mix audio;
6. connect overlays;
7. select encoders;
8. choose fast path vs re-encode path;
9. produce deterministic execution plan;
10. provide human-readable diagnostic output.

Never construct unsafe shell commands by string concatenation.

If FFmpeg CLI is used by a separate diagnostic/export adapter, arguments must be passed as structured process arguments.

---

# 21. Project file format

Use a human-readable versioned JSON project format for v1.

Example:

```json
{
  "format_version": 1,
  "project": {
    "name": "My Video",
    "width": 1080,
    "height": 1920,
    "fps": {"num": 30, "den": 1},
    "background": "#000000"
  },
  "assets": [],
  "tracks": [],
  "settings": {
    "default_image_duration": 5.0
  }
}
```

## Requirements

- explicit schema version;
- migrate older project versions;
- relative paths when possible;
- stable asset IDs;
- preserve missing-media references rather than deleting them;
- no absolute machine-specific paths unless needed;
- autosave recovery file.

---

# 22. Undo/redo

Use a command/history model rather than taking a complete deep copy of the entire project for every action.

Operations that should be undoable:
- import/remove asset reference;
- add clip;
- delete clip;
- trim;
- split;
- move;
- mute;
- volume;
- crop;
- transform;
- ratio change;
- add image;
- add audio;
- reorder tracks;
- export setting changes where project settings are persistent.

Undo/redo must not re-render media.

---

# 23. Keyboard shortcuts

Minimum:

| Shortcut | Action |
|---|---|
| Space | Play/Pause |
| I | Mark in |
| O | Mark out |
| S | Split |
| Delete | Delete selected |
| Ctrl+Z | Undo |
| Ctrl+Shift+Z | Redo |
| Ctrl+S | Save |
| Ctrl+O | Open/Import |
| Ctrl+E | Export |
| Left/Right | Step/seek |
| Shift+Left/Right | Larger seek |
| Home | Go start |
| End | Go end |
| J/K/L | Reverse/stop/forward playback where implemented |

All shortcuts must be discoverable in the UI and avoid conflicts with standard GNOME conventions.

---

# 24. Context menus

### Video clip

- Cut
- Split
- Trim
- Delete
- Duplicate
- Mute clip audio
- Detach audio (if supported)
- Add fade
- Crop
- Transform
- Speed
- Replace source
- Properties

### Audio clip

- Split
- Trim
- Delete
- Mute
- Volume
- Fade in
- Fade out
- Duplicate
- Properties

### Image clip

- Duration
- Fit
- Fill
- Crop
- Transform
- Opacity
- Replace image
- Delete

---

# 25. Accessibility and UX quality

Requirements:
- keyboard navigable controls;
- visible focus states;
- labels/tooltips for icon-only actions;
- sufficient contrast;
- no critical action represented by color alone;
- status messages for long operations;
- error messages that explain the next action.

Follow modern GNOME HIG principles and libadwaita patterns.

---

# 26. Error handling

Errors must be structured, not raw FFmpeg dumps in normal UI.

Example:

```text
Export failed

FluxCut could not encode this video with the selected hardware encoder.

Reason:
The selected codec/profile is not supported by the detected GPU.

[Use Software Encoding] [Change Codec] [View Technical Details]
```

Technical details may expose the FFmpeg/library error for debugging.

Never crash on:
- corrupt media;
- unsupported codec;
- missing source file;
- failed thumbnail;
- failed waveform;
- export cancellation;
- unsupported GPU path.

---

# 27. Security and robustness

- never execute arbitrary file contents as commands;
- never construct shell command strings from untrusted filenames;
- sanitize output paths;
- do not assume file extensions imply codec;
- validate media through actual probing;
- isolate unsafe FFI code;
- bound cache growth;
- clean temporary files on failure;
- protect against path traversal in project-relative asset restoration.

---

# 28. Packaging requirements

Primary target:

- Flatpak for reproducible user deployment.

Also prepare:

- native distribution package documentation for Fedora/Debian/Arch-style systems;
- optional source build instructions.

The build must document system dependencies for:

- GTK4;
- libadwaita;
- FFmpeg development libraries;
- Vulkan loader/headers;
- VA-API where available;
- Wayland development libraries.

Do not bundle enormous unrelated runtimes.

---

# 29. Suggested Rust workspace

```text
fluxcut/
├── Cargo.toml
├── Cargo.lock
├── crates/
│   ├── app/              # application bootstrap / lifecycle
│   ├── ui/               # GTK4/libadwaita widgets and views
│   ├── timeline/         # timeline state + custom rendering
│   ├── project/          # schema, persistence, migrations
│   ├── media/            # asset model and metadata
│   ├── ffmpeg-core/      # safe FFmpeg abstraction
│   ├── decode/            # playback/decode workers
│   ├── render/            # preview renderer/GPU bridge
│   ├── filters/           # editor graph -> FFmpeg graph
│   ├── audio/             # waveform + audio graph helpers
│   ├── export/            # render/export execution
│   ├── cache/             # thumbnail/waveform/proxy cache
│   ├── hardware/          # GPU/VAAPI/Vulkan capability detection
│   └── diagnostics/       # logs and performance diagnostics
├── resources/
│   ├── ui/
│   ├── icons/
│   ├── presets/
│   └── templates/
├── tests/
├── benches/
└── docs/
```

The agent may alter names if the actual dependency graph warrants it, but must preserve the separation of responsibilities.

---

# 30. State architecture

Recommended high-level state:

```text
AppState
├── WindowState
├── ProjectState
│   ├── ProjectSettings
│   ├── Assets
│   ├── Tracks
│   ├── Selection
│   └── History
├── PlaybackState
├── MediaRuntime
├── HardwareCapabilities
├── CacheState
└── ExportState
```

UI should observe state rather than owning the source of truth.

Avoid circular dependencies between UI and media engine.

---

# 31. Concurrency model

## Main/UI thread

Only:
- GTK operations;
- user events;
- state updates;
- lightweight scheduling;
- visual invalidation.

## Media worker pool

Handles:
- probing;
- decoding;
- frame extraction;
- thumbnails.

## Audio worker pool

Handles:
- waveform analysis;
- audio previews;
- audio conversion helpers.

## Export worker

Handles:
- filter graph execution;
- encoding;
- progress reporting.

## Cancellation

Every long-running operation should use a cancellation token/state shared with its worker.

Stale seeks must be cancellable/coalesced.

---

# 32. Performance budgets

These are targets for a representative modern Linux desktop/laptop; they are not guarantees for every machine.

## Startup

- initial application window should appear within ~1.5 seconds on a warm modern system;
- no media scan of the entire filesystem at startup.

## Import

- UI acknowledges a dropped file essentially immediately;
- metadata analysis occurs asynchronously.

## UI responsiveness

- no visible multi-second main-thread freeze;
- user interaction remains responsive during thumbnail/waveform generation and export;
- scrolling/zooming timeline should remain smooth with hundreds/thousands of clips where practical.

## Playback target

- aim for real-time playback for common H.264 1080p sources on supported modern hardware;
- when the target cannot be sustained, degrade preview quality/proxy mode rather than freezing the UI.

## Memory

- no unbounded decoded-frame retention;
- cache sizes configurable or automatically bounded;
- project size should not proportionally duplicate media contents in RAM.

## Export

- use hardware encoders where supported and stable;
- use fast path for remux/stream-copy operations;
- avoid repeated encode/decode stages.

---

# 33. Benchmark suite

Create reproducible benchmark media cases:

1. 1080p H.264 30fps, short clip;
2. 1080p H.264 60fps;
3. 4K H.264;
4. 4K HEVC;
5. 1080p VP9/WebM;
6. 1080p AV1 where available;
7. long audio file;
8. 1-hour screen recording;
9. project with 100+ cuts;
10. project with 500+ timeline clips;
11. mixed video + music + image overlays;
12. vertical 9:16 export.

Measure:
- startup time;
- import acknowledgement latency;
- first-frame latency;
- seek latency;
- preview FPS;
- CPU;
- GPU;
- RAM;
- thumbnail throughput;
- waveform generation throughput;
- export FPS;
- export wall time;
- output file size.

---

# 34. Definition of Done for performance

A feature is not complete merely because it works.

It is complete when:

- UI remains responsive;
- no avoidable media work occurs during editing;
- cancellation works;
- memory is bounded;
- errors are surfaced;
- tests cover the core behavior;
- benchmark impact is understood.

---

# 35. Testing strategy

## Unit tests

Test:
- time arithmetic;
- trim/split math;
- timeline placement;
- snapping;
- project serialization;
- migrations;
- audio mute/volume state;
- ratio calculations;
- crop calculations;
- output dimension calculations;
- filter graph compilation.

## Media integration tests

For known fixtures:
- import;
- decode;
- seek;
- thumbnail extraction;
- waveform extraction;
- export;
- probe exported file.

## Golden tests

For deterministic small media clips, compare:
- output dimensions;
- duration;
- audio stream count;
- selected metadata;
- pixel or perceptual results for selected effects where feasible.

Do not require byte-identical encoded outputs from hardware encoders unless the encoder is deterministic and that property is explicitly tested.

## UI tests

At minimum:
- open project;
- import media;
- add clip;
- split;
- mute;
- add audio;
- add image;
- change ratio;
- choose resolution;
- export.

---

# 36. Acceptance criteria for the end-to-end MVP

The MVP is accepted only when this exact scenario works:

### Scenario A — Social vertical video

1. Create project.
2. Select 9:16 / 1080x1920.
3. Import a 16:9 video.
4. Place video on timeline.
5. Change fit mode to Fill and reposition crop.
6. Split the clip.
7. Delete the middle section.
8. Mute the original video audio.
9. Import MP3/WAV music.
10. Put music beneath the video.
11. Reduce music volume.
12. Import a PNG logo.
13. Place logo over video for a selected duration.
14. Preview.
15. Export H.264 MP4 at 1080x1920.
16. Verify resulting dimensions and presence of music.

### Scenario B — Fast cut

1. Import a supported video.
2. Set in/out.
3. Export using the fast path when technically valid.
4. UI states whether export was stream-copy/remux or re-encode.
5. Output is playable.

### Scenario C — Audio replacement

1. Import video containing original audio.
2. Mute original audio.
3. Add external audio.
4. Trim external audio.
5. Add fade out.
6. Export.
7. Verify no original audio remains in the output and external audio is present.

### Scenario D — Image template

1. Import image.
2. Add it as a clip.
3. Apply an image template.
4. Change duration.
5. Preview.
6. Export.

---

# 37. Development phases

## Phase 0 — Environment and architecture proof

Tasks:
- [ ] inspect Linux environment;
- [ ] detect Wayland/X11;
- [ ] detect GTK4/libadwaita versions;
- [ ] detect FFmpeg 9.x libraries;
- [ ] detect Vulkan;
- [ ] detect DMA-BUF capability;
- [ ] detect VA-API/hardware codecs;
- [ ] generate technical environment report;
- [ ] create Rust workspace.

Deliverable:
- app launches as native GTK4 Wayland application;
- capability page/diagnostic command works.

## Phase 1 — Native shell

Tasks:
- [ ] main window;
- [ ] libadwaita styling;
- [ ] navigation/state layer;
- [ ] async file dialog;
- [ ] import UI;
- [ ] basic settings;
- [ ] logging.

Deliverable:
- polished empty editor shell.

## Phase 2 — Media ingestion

Tasks:
- [ ] FFmpeg initialization;
- [ ] probe assets;
- [ ] metadata model;
- [ ] thumbnail worker;
- [ ] waveform worker;
- [ ] cache;
- [ ] error states.

Deliverable:
- imported media appears quickly with metadata/thumbnail/waveform.

## Phase 3 — Timeline core

Tasks:
- [ ] timeline model;
- [ ] video/audio tracks;
- [ ] custom timeline renderer;
- [ ] clip selection;
- [ ] drag/move;
- [ ] trim;
- [ ] split;
- [ ] delete;
- [ ] undo/redo.

Deliverable:
- reliable basic NLE timeline.

## Phase 4 — Playback

Tasks:
- [ ] decode worker;
- [ ] play/pause;
- [ ] seek;
- [ ] frame stepping;
- [ ] preview surface;
- [ ] cancellation/coalescing;
- [ ] software fallback.

Deliverable:
- responsive preview for common media.

## Phase 5 — Audio

Tasks:
- [ ] audio tracks;
- [ ] import audio;
- [ ] waveform;
- [ ] clip mute;
- [ ] track mute;
- [ ] volume;
- [ ] fade;
- [ ] audio mixing;
- [ ] synchronization.

Deliverable:
- video + music workflow is complete.

## Phase 6 — Image + transforms

Tasks:
- [ ] image clips;
- [ ] overlays;
- [ ] templates;
- [ ] crop;
- [ ] scale;
- [ ] rotation;
- [ ] opacity;
- [ ] position.

Deliverable:
- image/video composition workflow.

## Phase 7 — Canvas and export

Tasks:
- [ ] project ratio;
- [ ] fit/fill modes;
- [ ] resolution presets;
- [ ] FPS settings;
- [ ] filter graph compiler;
- [ ] software encoding;
- [ ] hardware encoder detection;
- [ ] export progress;
- [ ] output verification.

Deliverable:
- stable export system.

## Phase 8 — Optimization

Tasks:
- [ ] hardware decode;
- [ ] hardware encode;
- [ ] GPU frame path;
- [ ] Vulkan optimization;
- [ ] DMA-BUF compatibility path;
- [ ] proxy mode;
- [ ] cache tuning;
- [ ] timeline performance benchmarks.

Deliverable:
- measurable performance improvements with benchmark report.

## Phase 9 — Packaging and release candidate

Tasks:
- [ ] Flatpak;
- [ ] desktop file;
- [ ] app icon;
- [ ] MIME associations;
- [ ] documentation;
- [ ] crash-safe project recovery;
- [ ] release QA matrix.

---

# 38. Implementation rules for the AI agent

1. **Build incrementally.** Never generate thousands of lines across every subsystem in one step.
2. **Compile after each architectural milestone.**
3. **Do not fake FFmpeg playback with a shell process as the final design.**
4. **Do not use GTK widgets as individual timeline clips.**
5. **Do not load an entire high-resolution video into memory.**
6. **Do not decode the whole file to generate thumbnails.**
7. **Do not regenerate waveform on every timeline redraw.**
8. **Do not run FFmpeg export synchronously in the UI thread.**
9. **Do not assume hardware acceleration is available.**
10. **Do not assume Wayland/Vulkan/DMA-BUF support is identical across systems.**
11. **Do not use unsafe code outside narrowly scoped media/platform FFI layers without justification.**
12. **Do not silently change project ratio or crop semantics.**
13. **Do not destroy source files.**
14. **Do not make project files dependent on temporary proxy paths.**
15. **Every new feature must include tests and a basic failure path.**

---

# 39. Suggested internal APIs

The exact Rust signatures may evolve, but the architecture should resemble:

```rust
trait MediaSource {
    fn metadata(&self) -> MediaMetadata;
}

trait Decoder {
    fn seek(&mut self, timestamp: Timestamp) -> Result<()>;
    fn next_frame(&mut self) -> Result<Option<DecodedFrame>>;
}

trait PreviewRenderer {
    fn submit(&mut self, frame: DecodedFrame) -> Result<()>;
}

trait ExportEngine {
    fn plan(&self, project: &Project) -> Result<ExportPlan>;
    fn run(&self, plan: ExportPlan, progress: ProgressSink) -> Result<OutputReport>;
}
```

The concrete APIs must prevent UI code from directly manipulating raw FFmpeg pointers.

---

# 40. Output preset schema

Presets should be data-driven rather than hard-coded UI branches.

Example conceptual schema:

```json
{
  "id": "shorts-1080p",
  "name": "YouTube Shorts 1080p",
  "container": "mp4",
  "video_codec": "h264",
  "width": 1080,
  "height": 1920,
  "fps": 30,
  "audio_codec": "aac",
  "audio_channels": 2
}
```

Additional presets can be added without rewriting export logic.

---

# 41. Project template system

Templates should also be data-driven.

Template types:

- image position;
- duration;
- crop mode;
- scale;
- animation flags reserved for future;
- overlay opacity;
- default canvas ratio.

Example:

```json
{
  "id": "watermark-bottom-right",
  "type": "image_overlay",
  "position": {"x": 0.96, "y": 0.94},
  "anchor": "bottom-right",
  "scale": 0.12,
  "opacity": 0.85
}
```

---

# 42. Future roadmap after v1

Possible v2/v3 features:

- subtitles and caption styling;
- text overlays;
- transitions;
- more effects;
- scene/clip markers;
- silence detection and automatic cut suggestions;
- proxy quality controls;
- background blur for vertical video;
- animated image/text templates;
- waveform editing improvements;
- audio ducking;
- optional speech-to-text integration;
- render queue;
- batch export;
- command-line project renderer;
- plugin/effect API.

These are explicitly secondary to the v1 editing/export pipeline.

---

# 43. Definition of product success

FluxCut succeeds when a user can open a video and complete a simple project noticeably faster than expected, without the application feeling heavy.

Qualitative success:

> “It feels instant.”

Quantitative success metrics:

- first interactive UI appears quickly;
- no UI freeze during media analysis/export;
- fast seek for supported media;
- successful real-time 1080p preview on representative supported systems;
- common 1080p vertical export succeeds reliably;
- fast-path cuts avoid re-encoding when technically valid;
- CPU/RAM usage is bounded and documented;
- benchmark suite is reproducible.

---

# 44. Final architecture summary

```text
                         FLUXCUT
                            │
                ┌───────────┴───────────┐
                │       GTK4 UI         │
                │      libadwaita       │
                └───────────┬───────────┘
                            │
                    Project / Commands
                            │
                ┌───────────▼───────────┐
                │     Editor Engine     │
                │ timeline / clips /   │
                │ tracks / transforms  │
                └───────────┬───────────┘
                            │
             ┌──────────────┼──────────────┐
             │              │              │
        Preview Engine   Filter Compiler  Export Engine
             │              │              │
             └──────────────┼──────────────┘
                            │
                       FFmpeg Core
                            │
       ┌────────────────────┼────────────────────┐
       │                    │                    │
   libavformat          libavcodec          libavfilter
       │                    │                    │
       └────────────────────┼────────────────────┘
                            │
                  Hardware / Software
                     decode + encode
                            │
                  Vulkan / DMA-BUF path
                            │
                       Wayland display
```

The key architectural idea is that **GTK4 owns the application interface, the editor engine owns semantic project state, and FFmpeg owns media processing**. The boundaries between these systems should remain explicit.

---

# 45. Research notes / source basis

This PRD's technical direction was checked against current documentation available on 2026-09-07, especially:

- FFmpeg 9.0.1 release information and current documentation;
- FFmpeg filter documentation for crop, scale, overlay, volume, amix, atempo and timestamp manipulation;
- FFmpeg codec/hardware acceleration documentation;
- GTK4 4.23.x documentation and Wayland backend documentation;
- GTK4 GSK/Vulkan rendering documentation;
- GTK4 DMA-BUF APIs;
- current gtk4-rs and ffmpeg-next/ffmpeg-sys-next package information;
- GNOME Human Interface Guidelines for GTK4/libadwaita applications;
- PRD guidance emphasizing problem, goals/non-goals, user stories, requirements, acceptance criteria, metrics and release scope.

The implementation agent must re-check dependency versions and actual system capabilities before locking versions in code. Version numbers in this document are technical targets, not permission to assume a particular distribution has them installed.

---

# 46. Agent handoff checklist

Before writing substantial implementation code, the agent must output:

- [ ] architecture diagram;
- [ ] crate/dependency plan;
- [ ] system dependency matrix;
- [ ] FFmpeg integration choice;
- [ ] playback strategy;
- [ ] timeline rendering strategy;
- [ ] GPU capability detection strategy;
- [ ] project schema proposal;
- [ ] milestone order;
- [ ] risks and mitigations;
- [ ] benchmark plan.

Then implement **Phase 0 only** and prove the build before proceeding.


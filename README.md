# Monora (Simple Recorder)

<p align="center">
  <img src="assets/brand/monora-icon.svg" width="96" height="96" alt="Monora logo" />
</p>

[![CI](https://github.com/YOUR_GITHUB_USERNAME/Monora/actions/workflows/ci.yml/badge.svg)](https://github.com/YOUR_GITHUB_USERNAME/Monora/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![GitHub Pages](https://img.shields.io/badge/docs-GitHub%20Pages-245edc)](https://YOUR_GITHUB_USERNAME.github.io/Monora/)

Lightweight, portable **Windows screen recorder** built with **Tauri 2**, **Rust**, and **TypeScript**.

**Website (downloads):** [https://YOUR_GITHUB_USERNAME.github.io/Monora/](https://YOUR_GITHUB_USERNAME.github.io/Monora/)

Record one or more physical monitors with system audio and/or microphone, encode to **H.264 + AAC** in **MP4**, and save to your chosen folder—without FFmpeg, Python, Node.js, or an installer on the target machine (portable ZIP).

## Features

- Multi-monitor capture (physical displays via Windows Graphics Capture)
- Combined desktop layout or separate files per monitor
- WASAPI loopback system audio + microphone selection
- Simple audio mix (system + mic) with clipping protection
- Hardware H.264 via Media Foundation when available
- 30 / 60 FPS, quality presets (Balanced / High / Very High)
- System tray + global shortcut `Ctrl+Shift+R`
- Portable-friendly layout (`config/`, `logs/` next to the executable)

## Requirements

- **Windows 10 or Windows 11** (64-bit)
- **WebView2** runtime (usually already installed on current Windows; Tauri can bootstrap it if missing)
- For development: Node.js, Rust (MSVC toolchain), Windows C++ Build Tools

## Download

Prebuilt binaries are published on [GitHub Releases](https://github.com/YOUR_GITHUB_USERNAME/Monora/releases):

| Asset | Description |
|-------|-------------|
| `SimpleRecorder-portable.zip` | Unzip and run — no installer |
| `SimpleRecorder_*_x64-setup.exe` | NSIS installer |

Replace `YOUR_GITHUB_USERNAME` in links after you create the repository.

## Development

```bash
npm install
npm run tauri dev
```

## Release build (local)

```bash
npm install
npm run tauri build
./scripts/package-portable.ps1
```

Typical outputs:

- NSIS installer: `src-tauri/target/release/bundle/nsis/SimpleRecorder_*_x64-setup.exe`
- Portable ZIP: `dist-portable/SimpleRecorder-portable.zip`

## Releasing on GitHub

1. Push the repository to GitHub.
2. **Enable GitHub Pages (required once per repo).** If deploy fails with `Failed to create deployment (status: 404)`, Pages is not enabled yet:
   - Open **Settings → Pages** (e.g. `https://github.com/<owner>/Monora/settings/pages`).
   - Under **Build and deployment**, set **Source** to **GitHub Actions** (not “Deploy from a branch”).
   - Save, then re-run the **Deploy GitHub Pages** workflow (**Actions** tab → workflow → **Run workflow**).
   - After the first successful deploy, the site is at `https://<owner>.github.io/Monora/`.
3. Create and push a version tag (must match `version` in `src-tauri/tauri.conf.json`):

   ```bash
   git tag v1.0.0
   git push origin v1.0.0
   ```

   The [Release workflow](.github/workflows/release.yml) builds on Windows and uploads the portable ZIP and NSIS installer.

## Default output

Videos are written to:

`%USERPROFILE%\Videos\SimpleRecorder\`

Filenames look like:

`SimpleRecorder_2026-09-26_00-45-32.mp4`

## Recording modes

| Mode | Behavior |
|------|----------|
| **Combined** | One MP4 with monitors placed by Windows desktop coordinates |
| **Separate** | One MP4 per selected monitor (`_Monitor-1`, `_Monitor-2`, …) |

## Audio

- **System audio**: WASAPI loopback (no Stereo Mix required)
- **Microphone**: selectable input device
- Either or both can be enabled; mixed into a single AAC track when both are on

## Hardware encoding

The encoder requests Media Foundation hardware transforms (`MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS`). Actual encoder choice depends on Windows and GPU drivers (Intel / AMD / NVIDIA). If hardware encoding is unavailable, Media Foundation falls back to a software path.

## Logs & settings

| Path | Purpose |
|------|---------|
| `<exe dir>\logs\simple-recorder.log` | Technical log |
| `<exe dir>\config\settings.json` | UI preferences |

## Known limitations

- No pause, webcam, streaming, or per-window capture
- Combined mode may include empty canvas padding when monitor sizes/positions differ
- Monitor disconnect during recording surfaces an error rather than silent recovery
- Global hotkeys may require the app to be allowed to register shortcuts (varies by environment)
- Very high resolutions at 60 FPS depend on GPU encoder capability and disk speed

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Please read our [Code of Conduct](CODE_OF_CONDUCT.md).

## License

[MIT](LICENSE) — Copyright (c) Monora Contributors.

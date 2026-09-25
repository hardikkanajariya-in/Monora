# Monora

<p align="center">
  <img src="assets/brand/monora-icon.svg" width="96" height="96" alt="Monora" />
</p>

Windows screen recorder — Tauri 2, Rust, TypeScript.

Downloads: [hardikkanajariya-in.github.io/Monora](https://hardikkanajariya-in.github.io/Monora/) · [Releases](https://github.com/hardikkanajariya-in/Monora/releases)

Captures physical monitors with optional system and microphone audio, encodes H.264 + AAC to MP4. Portable layout or NSIS installer.

## Features

- Multi-monitor capture (Windows Graphics Capture)
- Combined canvas or one file per monitor
- WASAPI loopback and microphone input, mixed to one AAC track
- Media Foundation hardware H.264 when available
- 30 / 60 FPS, quality presets
- System tray, `Ctrl+Shift+R` hotkey
- `config/` and `logs/` beside the executable

## Requirements

- Windows 10/11 x64
- WebView2
- Dev: Node.js, Rust (MSVC), Windows SDK

## Build

```bash
npm install
npm run tauri dev
```

Release:

```bash
npm run tauri build
./scripts/package-portable.ps1
```

Tag `v*` on `main` runs the Windows release workflow (portable ZIP + NSIS).

## Output

Default folder: `%USERPROFILE%\Videos\SimpleRecorder\`

Example: `SimpleRecorder_2026-09-26_00-45-32.mp4`

| Mode | Behavior |
|------|----------|
| Combined | One MP4 aligned to desktop coordinates |
| Separate | One MP4 per monitor (`_Monitor-1`, …) |

## Logs & settings

| Path | Purpose |
|------|---------|
| `<exe>\logs\simple-recorder.log` | Log file |
| `<exe>\config\settings.json` | Preferences |

## License

MIT — see [LICENSE](LICENSE).

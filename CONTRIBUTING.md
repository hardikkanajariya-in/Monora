# Contributing to Monora

Thank you for helping improve Monora (Simple Recorder). This project is a Tauri 2 desktop app with a Rust backend and a TypeScript/Vite frontend.

## Development setup

1. Install [Node.js](https://nodejs.org/) (LTS), [Rust](https://rustup.rs/) (MSVC toolchain), and [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) with the Windows 10/11 SDK.
2. Clone the repository and install dependencies:

   ```bash
   npm install
   npm run tauri dev
   ```

## Pull requests

1. Open an issue for large changes so we can align on approach.
2. Keep PRs focused; include a short description of **why** the change is needed.
3. Run `npm run build` and, when touching the native layer, `npm run tauri build` on Windows before submitting.
4. Follow existing code style in the files you edit.

## Reporting bugs

Use the [bug report template](.github/ISSUE_TEMPLATE/bug_report.yml) and include Windows version, GPU, monitor layout, and steps to reproduce.

## License

By contributing, you agree that your contributions will be licensed under the [MIT License](LICENSE).

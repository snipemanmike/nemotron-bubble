<div align="center">

# Nemotron Bubble

**A tiny, beautiful push‑to‑talk dictation bubble for Windows.**

Press `Ctrl+Space`, speak, and your words appear at the cursor — powered by **on‑device** NVIDIA Nemotron speech recognition.

<img src="docs/demo.gif" width="460" alt="Nemotron Bubble live waveform" />

![Platform](https://img.shields.io/badge/platform-Windows%2010%2F11-0078D6?logo=windows&logoColor=white)
![Built with Rust](https://img.shields.io/badge/built%20with-Rust-CE412B?logo=rust&logoColor=white)
![100% offline](https://img.shields.io/badge/100%25-offline-2ea44f)
![License: MIT](https://img.shields.io/badge/license-MIT-blue)

</div>

---

## ✨ Features

- **Push‑to‑talk** — `Ctrl+Space` starts and stops dictation from any app.
- **Types as you speak** — words stream straight into the focused window as real keystrokes; your **clipboard is never touched** until you stop.
- **Private & offline** — runs the Nemotron ONNX model locally. No internet, no accounts, no telemetry.
- **Live waveform bubble** — a smooth, anti‑aliased floating mic meter you can drag anywhere.
- **Tray mode** — hide the bubble and watch the waveform pulse right in the taskbar tray instead.
- **Recent dictations** — scrollable history with one‑click *Copy Latest*.
- **One clean clipboard copy on stop** — with optional auto‑paste.
- **Starts with Windows**, soft sound cues, model preloading — all toggleable.

## 📸 Settings

<img src="docs/settings.png" width="620" alt="Settings window" />

## 🚀 Quick start

> Requires **Windows 10/11** and the [Rust toolchain](https://rustup.rs) (MSVC).

```powershell
git clone https://github.com/snipemanmike/nemotron-bubble.git
cd nemotron-bubble

# One-time: download the Nemotron speech model (~2.4 GB) into models\nemotron\
.\scripts\download-nemotron.ps1

# Build and run
cargo run --release
```

Prefer the multilingual model? Run `.\scripts\download-nemotron.ps1 -Multilingual`.
You can also point at a custom folder with `$env:NEMOTRON_MODEL_DIR = "C:\path\to\nemotron"`.

## ⌨️ Controls

| Action | How |
| --- | --- |
| Start / stop dictation | `Ctrl+Space` |
| Open / close settings | click the bubble **or** the tray icon |
| Move the bubble | drag it |
| Tray menu (start, show/hide, quit) | right‑click the tray icon |

## 🧩 How it works

Nemotron Bubble is a single‑file Rust app built directly on the Win32 API — no Electron, no web view, ~5 MB binary. Microphone audio is captured with [`cpal`](https://crates.io/crates/cpal), streamed through the [`parakeet-rs`](https://crates.io/crates/parakeet-rs) Nemotron ONNX model, and injected into the focused window as Unicode keystrokes. The floating bubble is a **per‑pixel‑alpha layered window** drawn by a small software canvas with signed‑distance anti‑aliasing, so the pill, shadow, and waveform stay crisp and smooth.

## 🛠️ Build

```powershell
cargo build --release            # the app

# Regenerate the README art (bubble.png, demo.gif, settings.png)
cargo run --release --features assets -- --render-assets docs
```

## 🙏 Credits

- Speech recognition via [parakeet‑rs](https://crates.io/crates/parakeet-rs) and NVIDIA's Nemotron streaming ASR.
- Model weights hosted on Hugging Face.

## 📄 License

MIT — see [LICENSE](LICENSE).

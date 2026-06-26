# Nemotron Bubble for macOS

This branch adds a native macOS menu-bar dictation app while keeping the existing
Windows Rust app unchanged.

The macOS app uses the same `parakeet-rs` / NVIDIA Nemotron ONNX transcription
path as the Windows app. The app shell is Swift/AppKit because the original UI is
direct Win32 code, but transcription, chunking, final flush, and silence auto-stop
run through the bundled Rust `nemotron-engine` helper.

## Build and run

```bash
cd macos
./build-macos-app.sh
open ".build/release/Nemotron Bubble.app"
```

The app lives in the menu bar and shows a floating bubble by default.

Download the model first if `models/nemotron` does not exist:

```bash
../scripts/download-nemotron.sh
```

## Permissions

macOS will ask for:

- Microphone access for recording dictation.
- Accessibility access if you keep `Paste on Stop` enabled, because macOS
  requires it before an app can send `Cmd-V` to the active app.

If auto-paste is not allowed yet, the transcript is still copied to the
clipboard so you can paste it manually.

## Controls

| Action | How |
| --- | --- |
| Start / stop dictation | `Ctrl-Space`, menu item, or click the bubble |
| Move bubble | Drag it |
| Open settings | Click the bubble or choose Settings from the menu bar item |
| Copy latest transcript | Menu bar item |
| Paste latest transcript | Menu bar item |
| Toggle auto-copy / auto-paste / live typing / bubble | Settings window or menu bar items |

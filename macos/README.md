# Nemotron Bubble for macOS

This branch adds a native macOS menu-bar dictation app while keeping the existing
Windows Rust app unchanged.

The macOS app uses Apple's Speech framework instead of the Windows Nemotron ONNX
path. The current Windows implementation is built directly on Win32 APIs, so the
Mac shell is implemented as a small Swift/AppKit app with the same core workflow:
press `Ctrl-Space`, speak, stop, then copy or paste the transcript.

## Build and run

```bash
cd macos
./build-macos-app.sh
open ".build/release/Nemotron Bubble.app"
```

The app lives in the menu bar and shows a floating bubble by default.

## Permissions

macOS will ask for:

- Microphone access for recording dictation.
- Speech Recognition access for transcription.
- Accessibility access if you keep `Paste on Stop` enabled, because macOS
  requires it before an app can send `Cmd-V` to the active app.

If auto-paste is not allowed yet, the transcript is still copied to the
clipboard so you can paste it manually.

## Controls

| Action | How |
| --- | --- |
| Start / stop dictation | `Ctrl-Space`, menu item, or click the bubble |
| Move bubble | Drag it |
| Copy latest transcript | Menu bar item |
| Paste latest transcript | Menu bar item |
| Toggle auto-copy / auto-paste / bubble | Menu bar items |

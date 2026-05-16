# Cursor Gesture Assistant

Windows-first Rust workspace for a low-latency gesture assistant that activates on a left-click hold plus mouse wiggle, captures selectable text first, and falls back to screenshots or images when text is not available.

## Current shape

- `cursor-core`: shared config and IPC types
- `cursor-helper`: background worker that will own gesture detection, capture, and Gemini requests
- `cursor-settings`: lightweight settings UI and API-key setup
- `cursor-tray`: tray launcher for the helper and settings UI

## First implementation slice

- Shared JSON config model with `AppConfig`
- Local TCP command protocol for helper control
- Settings window with API key, startup mode, and gesture controls
- Helper process stub that accepts `Ping`, `GetStatus`, `UpdateConfig`, `SimulateGesture`, and `Shutdown`
- Tray launcher shell that will host the actual tray icon and menu next

## v1 goals

- API-key-first setup
- No local history
- Text selection first, screenshot/image fallback second
- Low RAM usage and fast response
- Windows first, Linux later

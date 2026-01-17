# Hotkey-Type

A cross-platform desktop dictation tool: press a global hotkey to start/stop recording → transcribe using your own API key via cloud services → automatically paste results to the current cursor position. Features a small floating panel, tray control, and history.

## 🎯 Product Vision

One-line description: A cross-platform desktop dictation tool that uses global hotkeys to start/stop recording, transcribes via cloud APIs (OpenAI/Google) with your own API key, and automatically pastes results to the current cursor position. Includes a small floating panel, tray control, and history.

## 🛠️ Tech Stack

- **Desktop Shell**: Tauri v2
- **UI**: React + TypeScript + Vite
- **Storage**: SQLite (or KV + JSON for v1; SQLite for later versions)
- **Provider**: OpenAI / Google (Direct Cloud API)
- **Audio**: Local recording (mono, 16k/48k, passed to provider)

## ✅ Completed Features

### Core Infrastructure
- [x] Tauri v2 project setup with React + TypeScript
- [x] Small draggable panel window
  - Always-on-top
  - Transparent background
  - No window decorations
  - Size: 360x140px
  - macOS Private API enabled for transparency
- [x] Global hotkey registration
  - Default: `Ctrl+Shift+T`
  - Cross-platform support
  - Proper event handling
- [x] System tray integration
  - Tray icon with menu
  - Menu items: Start/Stop Dictation, Show Panel, Quit
- [x] Event system
  - Rust backend emits events to frontend
  - Frontend listens and updates UI state
  - Proper event permissions configured
- [x] State management
  - Phase transitions: `IDLE` → `RECORDING` → `PROCESSING` → `IDLE`
  - Debouncing to prevent duplicate triggers
  - Visual feedback in panel

### Permissions & Capabilities
- [x] Global shortcut permissions
- [x] Clipboard manager permissions
- [x] Event listening permissions
- [x] macOS accessibility permissions (for global shortcuts)

## 🚧 In Progress / TODO

### MVP Core Features
- [ ] Audio recording functionality
  - Start/stop recording on hotkey press
  - Audio format: mono, 16k/48k
  - Save audio buffer for transcription
- [ ] Cloud transcription integration
  - OpenAI Whisper API
  - Google Speech-to-Text API
  - Error handling and retry logic
- [ ] Auto-paste functionality
  - Detect current cursor position
  - Paste transcribed text automatically
  - Fallback to clipboard copy
- [ ] Settings panel
  - Provider selection (OpenAI / Google)
  - API key input and secure storage (Keychain/Credential Vault)
  - Custom hotkey configuration
  - Language selection (auto-detect / specific language)
- [ ] History feature
  - Store: timestamp, text, provider, model, duration
  - Recent 50 entries
  - Clear history option
  - Basic history UI

### User Experience
- [ ] Recording animation in panel
- [ ] Processing animation
- [ ] Success/error toast notifications
- [ ] One-click retry on failure
- [ ] Panel drag functionality (currently static)

## 📦 Installation

```bash
# Install dependencies
npm install

# Run in development mode
npm run tauri dev

# Build for production
npm run tauri build
```

## 🔧 Configuration

### macOS Permissions

On macOS, you need to grant accessibility permissions for global shortcuts:

1. Open **System Settings** → **Privacy & Security** → **Accessibility**
2. Add your app to the list and enable it

### Current Hotkey

Default hotkey: `Ctrl+Shift+T`

To change the hotkey, modify `src-tauri/src/lib.rs` (will be configurable via settings in future).

## 📁 Project Structure

```
hotkey-type/
├── src/                    # React frontend
│   ├── App.tsx            # Main app component with state management
│   └── ...
├── src-tauri/             # Rust backend
│   ├── src/
│   │   └── lib.rs        # Main Rust logic (hotkeys, tray, events)
│   ├── capabilities/     # Tauri permissions
│   │   └── default.json  # Event, global-shortcut, clipboard permissions
│   └── tauri.conf.json   # Tauri configuration
└── ...
```

## 🎨 Current UI States

- **IDLE**: App is ready, waiting for hotkey
- **RECORDING**: Currently recording audio (visual feedback needed)
- **PROCESSING**: Transcribing audio (visual feedback needed)

## 🔐 Security Notes

- API keys will be stored securely using platform-native keychains:
  - macOS: Keychain
  - Windows: Credential Vault
  - Linux: Secret Service API

## 📝 Development Notes

### Event Flow

1. User presses global hotkey (`Ctrl+Shift+T`)
2. Rust handler in `lib.rs` detects the shortcut
3. Event `dictation-toggle` is emitted to frontend
4. React component in `App.tsx` receives event
5. State transitions: `IDLE` → `RECORDING` → `PROCESSING` → `IDLE`

### Adding New Features

- **Audio recording**: Add Tauri audio plugin or native Rust audio capture
- **API integration**: Add HTTP client (reqwest) for API calls
- **Settings**: Create settings window/panel with form inputs
- **History**: Add SQLite database or JSON file storage

## 🤝 Contributing

This is an MVP project. Contributions welcome for:
- Audio recording implementation
- Cloud API integration
- UI/UX improvements
- Cross-platform testing

## 📄 License

[Add your license here]

---

**Status**: MVP in development - Core infrastructure complete, audio/transcription features pending

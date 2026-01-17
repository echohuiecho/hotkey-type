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
  - Menu items: Start/Stop Dictation, Show Panel, Settings, Quit
- [x] Event system
  - Rust backend emits events to frontend
  - Frontend listens and updates UI state
  - Proper event permissions configured
- [x] State management
  - Phase transitions: `IDLE` → `RECORDING` → `TRANSCRIBING` → `PASTING` → `DONE`
  - Debouncing to prevent duplicate triggers
  - Visual feedback in panel

### Audio Recording
- [x] Audio recording functionality
  - Start/stop recording on hotkey press
  - Audio format: mono, 16-bit PCM WAV
  - Uses `cpal` for cross-platform audio capture
  - Uses `hound` for WAV file writing
  - Saves to app cache directory

### Cloud Transcription
- [x] OpenAI Whisper API integration
  - Supports `/v1/audio/transcriptions` endpoint
  - Multipart form data upload
  - Error handling and status reporting
  - Model: `whisper-1` (default)

### Auto-Paste Functionality
- [x] Automatic paste after transcription
  - Writes to clipboard first (fallback)
  - Simulates ⌘V (macOS) or Ctrl+V (Windows/Linux)
  - Uses `enigo` for keyboard simulation
  - Falls back to clipboard if paste simulation fails

### Settings Panel
- [x] Settings UI
  - OpenAI API Key input (password field)
  - Settings stored in `app_config_dir/settings.json`
  - Accessible via tray menu or panel button
  - Auto-reloads settings after save

### Permissions & Capabilities
- [x] Global shortcut permissions
- [x] Clipboard manager permissions
- [x] Event listening permissions
- [x] macOS accessibility permissions (for global shortcuts)
- [x] macOS microphone permissions (`NSMicrophoneUsageDescription` in Info.plist)

## 🚧 In Progress / TODO

### Future Enhancements
- [ ] Additional transcription providers
  - Google Speech-to-Text API
  - Azure Speech Services
  - Local Whisper models
- [ ] Advanced settings
  - Provider selection (OpenAI / Google / etc.)
  - Custom hotkey configuration
  - Language selection (auto-detect / specific language)
  - Model selection for OpenAI
- [ ] History feature
  - Store: timestamp, text, provider, model, duration
  - Recent 50 entries
  - Clear history option
  - Basic history UI
  - Export history

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

### First-Time Setup

1. **Set OpenAI API Key**:
   - Click the ⚙️ button in the panel, or
   - Right-click the tray icon → Settings
   - Enter your OpenAI API key (get it from [OpenAI Platform](https://platform.openai.com/api-keys))
   - Click Save

2. **macOS Permissions**:
   - **Microphone**: The app will prompt you on first use
   - **Accessibility** (for auto-paste):
     - Open **System Settings** → **Privacy & Security** → **Accessibility**
     - Add your app to the list and enable it
     - This allows the app to simulate ⌘V for automatic pasting

### Current Hotkey

Default hotkey: `Ctrl+Shift+T`

- Press once to start recording
- Press again to stop recording and transcribe
- The transcribed text will be automatically pasted at your cursor position

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

## 🎨 UI States

- **IDLE**: App is ready, waiting for hotkey
- **RECORDING**: Currently recording audio from microphone
- **TRANSCRIBING**: Sending audio to OpenAI API for transcription
- **PASTING**: Automatically pasting transcribed text
- **DONE**: Transcription complete and pasted (or copied to clipboard)
- **ERROR**: An error occurred (check console for details)

## 🔐 Security Notes

- **API Key Storage**:
  - Currently stored in `app_config_dir/settings.json` (local file)
  - Future: Will use platform-native keychains:
    - macOS: Keychain
    - Windows: Credential Vault
    - Linux: Secret Service API
- **Privacy**:
  - Audio is recorded locally and sent only to OpenAI API
  - No audio data is stored permanently (temporary WAV files are deleted)
  - API key is never shared or transmitted except to OpenAI API

## 📝 Development Notes

### Event Flow

1. User presses global hotkey (`Ctrl+Shift+T`)
2. Rust handler in `lib.rs` detects the shortcut
3. Event `dictation-toggle` is emitted to frontend
4. React component in `App.tsx` receives event
5. **If IDLE**:
   - Calls `start_recording()` → state: `RECORDING`
   - Audio is captured via `cpal` and written to WAV file
6. **If RECORDING**:
   - Calls `stop_recording()` → state: `TRANSCRIBING`
   - WAV file is sent to OpenAI API via `openai_transcribe()`
   - State: `PASTING` → calls `paste_text()` to simulate paste
   - State: `DONE` → shows success message

### Key Dependencies

- **Audio**: `cpal` (0.15) for capture, `hound` (3.5) for WAV writing
- **HTTP**: `reqwest` (0.12) with multipart support for OpenAI API
- **Input Simulation**: `enigo` (0.2) for keyboard simulation
- **Threading**: `crossbeam-channel`, `parking_lot`, `tokio`

### Architecture Notes

- **Audio State**: Uses thread-local storage for `cpal::Stream` (not Send+Sync)
- **Settings**: Stored in JSON file in app config directory
- **Error Handling**: Comprehensive error messages shown in UI

## 🚀 Usage

1. **Start the app**: Run `npm run tauri dev` or build and run the app
2. **Set API key**: Open Settings and enter your OpenAI API key
3. **Start recording**: Press `Ctrl+Shift+T` (or use tray menu)
4. **Stop recording**: Press `Ctrl+Shift+T` again
5. **Auto-paste**: The transcribed text will be automatically pasted at your cursor

### Tips

- If auto-paste fails (e.g., no accessibility permission), the text is still copied to clipboard - just press ⌘V/Ctrl+V manually
- The app shows the current state in the panel
- Check the console for detailed error messages if something goes wrong

## 🤝 Contributing

This is an MVP project. Contributions welcome for:
- Additional transcription providers (Google, Azure, etc.)
- History feature implementation
- UI/UX improvements
- Cross-platform testing
- Performance optimizations

## 📄 License

[Add your license here]

---

**Status**: ✅ MVP Core Features Complete - Audio recording, OpenAI transcription, and auto-paste are working!

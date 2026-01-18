// src-tauri/src/lib.rs
use base64::Engine;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::{cell::RefCell, path::PathBuf, sync::Arc, thread};
use tauri::{Emitter, Manager, PhysicalPosition};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

#[cfg(desktop)]
use std::sync::OnceLock;

// Thread-local recorder state (cpal::Stream is not Send/Sync)
thread_local! {
  static RECORDER_STATE: RefCell<Option<Recorder>> = RefCell::new(None);
}

struct Recorder {
  path: PathBuf,
  // dropping the stream stops capture
  stream: cpal::Stream,
  // closing tx stops writer thread
  tx: crossbeam_channel::Sender<Vec<i16>>,
  writer_join: thread::JoinHandle<anyhow::Result<()>>,
  sample_rate: u32,
}

#[derive(Serialize)]
struct RecordingStopped {
  path: String,
  sample_rate: u32,
  duration_ms: u64,
}

#[tauri::command]
fn greet(name: &str) -> String {
  format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
fn start_recording(app: tauri::AppHandle) -> Result<String, String> {
  let already_recording = RECORDER_STATE.with(|state| state.borrow().is_some());
  if already_recording {
    return Err("Already recording".into());
  }

  // choose an app cache dir for temp wav
  let cache_dir = app
    .path()
    .app_cache_dir()
    .map_err(|e| format!("cache dir: {e}"))?;
  std::fs::create_dir_all(&cache_dir).map_err(|e| format!("mkdir: {e}"))?;
  let path = cache_dir.join(format!("dictation-{}.wav", uuid::Uuid::new_v4()));

  // Get settings to check for preferred input device
  let settings = get_settings(app.clone())?;

  let host = cpal::default_host();
  let device = if settings.input_device_name.is_empty() {
    // Use default device
    host
      .default_input_device()
      .ok_or("No default input device (mic)".to_string())?
  } else {
    // Find device by name
    match host
      .input_devices()
      .map_err(|e| format!("list devices: {e}"))?
      .find(|d| {
        d.name()
          .map(|n| n == settings.input_device_name)
          .unwrap_or(false)
      }) {
      Some(device) => device,
      None => {
        eprintln!(
          "Warning: Input device '{}' not found. Falling back to default device.",
          settings.input_device_name
        );
        // Fall back to default device
        host
          .default_input_device()
          .ok_or("No default input device (mic)".to_string())?
      }
    }
  };

  let device_name = device.name().unwrap_or_else(|_| "Unknown".to_string());
  eprintln!("Start recording: using input device: {}", device_name);

  let config = device
    .default_input_config()
    .map_err(|e| format!("default input config: {e}"))?;

  let sample_rate = config.sample_rate().0;
  let channels = config.channels() as usize;

  eprintln!("Start recording: sample_rate: {}, channels: {}, format: {:?}", sample_rate, channels, config.sample_format());

  let (tx, rx) = crossbeam_channel::unbounded::<Vec<i16>>();
  let path_for_writer = path.clone();

  // writer thread: write i16 PCM to WAV
  let writer_join = thread::spawn(move || -> anyhow::Result<()> {
    let spec = hound::WavSpec {
      channels: 1,
      sample_rate,
      bits_per_sample: 16,
      sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(&path_for_writer, spec)?;
    let mut total_samples = 0usize;
    while let Ok(chunk) = rx.recv() {
      total_samples += chunk.len();
      for s in chunk {
        writer.write_sample(s)?;
      }
    }
    eprintln!("Writer thread: wrote {} total samples", total_samples);
    writer.finalize()?;
    Ok(())
  });

  // audio callback: convert to mono i16 and send to writer
  let err_fn = |err| eprintln!("cpal stream error: {}", err);

  let tx_cb = tx.clone();
  let start_instant = std::time::Instant::now();
  let duration_ms_shared = Arc::new(Mutex::new(0u64));
  let duration_ms_cb = duration_ms_shared.clone();
  let chunks_received = Arc::new(Mutex::new(0usize));
  let chunks_received_cb = chunks_received.clone();

  let stream = match config.sample_format() {
    cpal::SampleFormat::F32 => device
      .build_input_stream(
        &config.into(),
        move |data: &[f32], _| {
          // update duration estimate
          *duration_ms_cb.lock() = start_instant.elapsed().as_millis() as u64;

          let mut mono = Vec::with_capacity(data.len() / channels);
          for frame in data.chunks(channels) {
            let v = frame[0].clamp(-1.0, 1.0);
            mono.push((v * i16::MAX as f32) as i16);
          }

          let chunk_num = {
            let mut count = chunks_received_cb.lock();
            *count += 1;
            *count
          };

          // Log first few chunks to verify audio is being captured
          if chunk_num <= 3 {
            let max_amp = mono.iter().map(|&s| s.abs()).max().unwrap_or(0);
            eprintln!("Audio chunk #{}: {} samples, max amplitude: {}", chunk_num, mono.len(), max_amp);
          }

          let _ = tx_cb.send(mono);
        },
        err_fn,
        None,
      )
      .map_err(|e| format!("build stream: {e}"))?,
    cpal::SampleFormat::I16 => device
      .build_input_stream(
        &config.into(),
        move |data: &[i16], _| {
          *duration_ms_cb.lock() = start_instant.elapsed().as_millis() as u64;

          let mut mono = Vec::with_capacity(data.len() / channels);
          for frame in data.chunks(channels) {
            mono.push(frame[0]);
          }

          let chunk_num = {
            let mut count = chunks_received_cb.lock();
            *count += 1;
            *count
          };

          // Log first few chunks to verify audio is being captured
          if chunk_num <= 3 {
            let max_amp = mono.iter().map(|&s| s.abs()).max().unwrap_or(0);
            eprintln!("Audio chunk #{}: {} samples, max amplitude: {}", chunk_num, mono.len(), max_amp);
          }

          let _ = tx_cb.send(mono);
        },
        err_fn,
        None,
      )
      .map_err(|e| format!("build stream: {e}"))?,
    cpal::SampleFormat::U16 => device
      .build_input_stream(
        &config.into(),
        move |data: &[u16], _| {
          *duration_ms_cb.lock() = start_instant.elapsed().as_millis() as u64;

          let mut mono = Vec::with_capacity(data.len() / channels);
          for frame in data.chunks(channels) {
            let v = frame[0] as i32 - 32768;
            mono.push(v as i16);
          }

          let chunk_num = {
            let mut count = chunks_received_cb.lock();
            *count += 1;
            *count
          };

          // Log first few chunks to verify audio is being captured
          if chunk_num <= 3 {
            let max_amp = mono.iter().map(|&s| s.abs()).max().unwrap_or(0);
            eprintln!("Audio chunk #{}: {} samples, max amplitude: {}", chunk_num, mono.len(), max_amp);
          }

          let _ = tx_cb.send(mono);
        },
        err_fn,
        None,
      )
      .map_err(|e| format!("build stream: {e}"))?,
    _ => return Err("Unsupported sample format".into()),
  };

  stream.play().map_err(|e| format!("stream play: {e}"))?;

  let recorder = Recorder {
    path: path.clone(),
    stream,
    tx,
    writer_join,
    sample_rate,
  };
  RECORDER_STATE.with(|state| {
    *state.borrow_mut() = Some(recorder);
  });

  Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
fn stop_recording() -> Result<RecordingStopped, String> {
  let rec = RECORDER_STATE
    .with(|state| state.borrow_mut().take())
    .ok_or("Not recording".to_string())?;

  let path = rec.path.clone();
  eprintln!("Stop recording: stopping stream and writer for {}", path.to_string_lossy());

  // stop capture by dropping stream, close writer by dropping tx
  drop(rec.stream);
  drop(rec.tx);

  // wait writer finalize
  rec
    .writer_join
    .join()
    .map_err(|_| "writer thread panicked".to_string())?
    .map_err(|e| format!("writer failed: {e}"))?;

  // On Windows, wait a bit for file system to catch up
  #[cfg(windows)]
  {
    std::thread::sleep(std::time::Duration::from_millis(200));
  }

  // Verify file exists and has content
  if !path.exists() {
    return Err(format!("Recorded file does not exist: {}", path.to_string_lossy()));
  }

  let file_size = std::fs::metadata(&path)
    .map_err(|e| format!("get file metadata: {e}"))?
    .len();

  eprintln!("Stop recording: file written, size: {} bytes", file_size);

  if file_size == 0 {
    return Err("Recorded file is empty".into());
  }

  // duration: best-effort using file size/time is OK for MVP; keep simple:
  // (you can store duration_ms in state if you want exact)
  let duration_ms = 0;

  Ok(RecordingStopped {
    path: path.to_string_lossy().to_string(),
    sample_rate: rec.sample_rate,
    duration_ms,
  })
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
struct AppSettings {
  provider: String,
  openai_api_key: String,
  google_api_key: String,
  google_language: String,
  input_device_name: String,
  panel_visible: bool,
}

impl Default for AppSettings {
  fn default() -> Self {
    Self {
      provider: "openai".to_string(),
      openai_api_key: String::new(),
      google_api_key: String::new(),
      google_language: "en-US".to_string(),
      input_device_name: String::new(), // Empty means use default
      panel_visible: true, // Default to visible
    }
  }
}

#[derive(Serialize)]
struct InputDevice {
  name: String,
  is_default: bool,
}

#[tauri::command]
fn show_panel(app: tauri::AppHandle) -> Result<(), String> {
  #[cfg(desktop)]
  {
    if let Some(panel) = app.get_webview_window("panel") {
      panel.show().map_err(|e| format!("show panel: {e}"))?;
      panel.set_focus().map_err(|e| format!("focus panel: {e}"))?;
    } else {
      return Err("Panel window not found".to_string());
    }
  }
  Ok(())
}

#[tauri::command]
fn hide_panel(app: tauri::AppHandle) -> Result<(), String> {
  #[cfg(desktop)]
  {
    if let Some(panel) = app.get_webview_window("panel") {
      panel.hide().map_err(|e| format!("hide panel: {e}"))?;
    } else {
      return Err("Panel window not found".to_string());
    }
  }
  Ok(())
}

#[tauri::command]
fn list_input_devices() -> Result<Vec<InputDevice>, String> {
  let host = cpal::default_host();
  let default_device = host.default_input_device();
  let default_name = default_device
    .as_ref()
    .and_then(|d| d.name().ok())
    .unwrap_or_default();

  let devices: Result<Vec<_>, _> = host
    .input_devices()
    .map(|devices| {
      devices
        .filter_map(|device| {
          device.name().ok().map(|name| InputDevice {
            is_default: name == default_name,
            name,
          })
        })
        .collect()
    })
    .map_err(|e| format!("list devices: {e}"));

  devices
}

#[derive(Serialize)]
struct TranscribeResponse {
  text: String,
}

#[tauri::command]
async fn openai_transcribe(
  audio_path: String,
  api_key: String,
  model: Option<String>,
  language: Option<String>,
  prompt: Option<String>,
) -> Result<TranscribeResponse, String> {
  let model = model.unwrap_or_else(|| "whisper-1".to_string());

  // OpenAI Speech-to-Text: POST /v1/audio/transcriptions (multipart file + model)
  // supports wav/webm/mp3/m4a etc.
  let url = "https://api.openai.com/v1/audio/transcriptions";

  eprintln!("OpenAI transcribe: reading file from {}", audio_path);

  // On Windows, ensure file is ready by checking existence and size
  let path = std::path::Path::new(&audio_path);
  if !path.exists() {
    return Err(format!("Audio file does not exist: {}", audio_path));
  }

  // Small delay on Windows to ensure file is fully flushed
  #[cfg(windows)]
  {
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
  }

  let file_bytes = tokio::fs::read(&audio_path)
    .await
    .map_err(|e| format!("read audio: {e}"))?;

  eprintln!("OpenAI transcribe: read {} bytes from file", file_bytes.len());

  if file_bytes.is_empty() {
    return Err("Audio file is empty".into());
  }

  let file_part = reqwest::multipart::Part::bytes(file_bytes)
    .file_name("audio.wav")
    .mime_str("audio/wav")
    .map_err(|e| format!("mime: {e}"))?;

  let mut form = reqwest::multipart::Form::new()
    .text("model", model)
    .part("file", file_part);

  if let Some(lang) = language {
    form = form.text("language", lang);
  }
  if let Some(p) = prompt {
    form = form.text("prompt", p);
  }
  // You can also set: response_format="json" (default) / "text"
  // We'll keep json and parse { text }.

  let client = reqwest::Client::new();
  let resp = client
    .post(url)
    .bearer_auth(api_key)
    .multipart(form)
    .send()
    .await
    .map_err(|e| format!("network: {e}"))?;

  let status = resp.status();
  eprintln!("OpenAI transcribe: response status {}", status);

  if !status.is_success() {
    let body = resp.text().await.unwrap_or_default();
    eprintln!("OpenAI transcribe: error response body: {}", body);
    return Err(format!("OpenAI error {status}: {body}"));
  }

  let v: serde_json::Value = resp.json().await.map_err(|e| format!("json: {e}"))?;
  eprintln!("OpenAI transcribe: response JSON: {:?}", v);

  let text = v
    .get("text")
    .and_then(|x| x.as_str())
    .unwrap_or("")
    .to_string();

  eprintln!("OpenAI transcribe: extracted text: '{}'", text);

  Ok(TranscribeResponse { text })
}

#[tauri::command]
async fn google_transcribe(
  audio_path: String,
  api_key: String,
  language: Option<String>,
  model: Option<String>,
  enable_automatic_punctuation: Option<bool>,
) -> Result<TranscribeResponse, String> {
  eprintln!("Google transcribe: reading file from {}", audio_path);

  // On Windows, ensure file is ready by checking existence
  let path = std::path::Path::new(&audio_path);
  if !path.exists() {
    return Err(format!("Audio file does not exist: {}", audio_path));
  }

  // Small delay on Windows to ensure file is fully flushed
  #[cfg(windows)]
  {
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
  }

  let file_bytes = tokio::fs::read(&audio_path)
    .await
    .map_err(|e| format!("read audio: {e}"))?;

  eprintln!("Google transcribe: read {} bytes from file", file_bytes.len());

  if file_bytes.is_empty() {
    return Err("Audio file is empty".into());
  }

  let wav_reader =
    hound::WavReader::open(&audio_path).map_err(|e| format!("wav open: {e}"))?;
  let spec = wav_reader.spec();
  eprintln!("Google transcribe: WAV spec - channels: {}, sample_rate: {}, bits_per_sample: {}", spec.channels, spec.sample_rate, spec.bits_per_sample);

  if spec.bits_per_sample != 16 {
    return Err("Google Speech-to-Text requires 16-bit LINEAR16 audio".into());
  }

  // Check if audio contains actual sound (not just silence)
  let samples: Vec<i16> = wav_reader.into_samples::<i16>()
    .filter_map(|s| s.ok())
    .collect();

  if samples.is_empty() {
    return Err("Audio file contains no samples".into());
  }

  // Check if audio is mostly silent (all samples near zero)
  let max_amplitude = samples.iter()
    .map(|&s| s.abs() as u32)
    .max()
    .unwrap_or(0);

  eprintln!("Google transcribe: audio samples: {}, max amplitude: {}", samples.len(), max_amplitude);

  // If max amplitude is very low, the audio is likely silent
  if max_amplitude < 100 {
    eprintln!("Google transcribe: WARNING - audio appears to be silent or very quiet (max amplitude: {})", max_amplitude);
  }

  let encoded_audio = base64::engine::general_purpose::STANDARD.encode(file_bytes);
  let language_code = language.unwrap_or_else(|| "en-US".to_string());
  let model = model.unwrap_or_else(|| "default".to_string());
  let enable_automatic_punctuation = enable_automatic_punctuation.unwrap_or(true);

  let body = serde_json::json!({
    "audio": { "content": encoded_audio },
    "config": {
      "enableAutomaticPunctuation": enable_automatic_punctuation,
      "encoding": "LINEAR16",
      "languageCode": language_code,
      "model": model,
      "sampleRateHertz": spec.sample_rate
    }
  });

  let url = format!(
    "https://speech.googleapis.com/v1p1beta1/speech:recognize?key={}",
    api_key
  );
  let client = reqwest::Client::new();
  let resp = client
    .post(url)
    .json(&body)
    .send()
    .await
    .map_err(|e| format!("network: {e}"))?;

  let status = resp.status();
  eprintln!("Google transcribe: response status {}", status);

  if !status.is_success() {
    let body = resp.text().await.unwrap_or_default();
    eprintln!("Google transcribe: error response body: {}", body);
    return Err(format!("Google Speech error {status}: {body}"));
  }

  let v: serde_json::Value = resp.json().await.map_err(|e| format!("json: {e}"))?;
  eprintln!("Google transcribe: response JSON: {:?}", v);

  // Check if results field exists
  let text = if let Some(results) = v.get("results") {
    if let Some(results_array) = results.as_array() {
      if results_array.is_empty() {
        eprintln!("Google transcribe: results array is empty - no speech detected");
        return Err("No speech detected in audio. The audio may be silent or too quiet.".into());
      }
      results_array
        .get(0)
        .and_then(|r| r.get("alternatives"))
        .and_then(|a| a.as_array())
        .and_then(|a| a.get(0))
        .and_then(|a| a.get("transcript"))
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .to_string()
    } else {
      eprintln!("Google transcribe: results is not an array");
      return Err("Invalid response format: results is not an array".into());
    }
  } else {
    eprintln!("Google transcribe: no 'results' field in response - no speech detected");
    return Err("No speech detected in audio. The audio may be silent, too quiet, or the language may not match.".into());
  };

  eprintln!("Google transcribe: extracted text: '{}'", text);

  Ok(TranscribeResponse { text })
}

#[tauri::command]
fn paste_text(app: tauri::AppHandle, text: String) -> Result<bool, String> {
  // 1) Always write clipboard first (fallback)
  // Use clipboard manager plugin API
  use tauri_plugin_clipboard_manager::ClipboardExt;
  app
    .clipboard()
    .write_text(text.clone())
    .map_err(|e| format!("clipboard: {e}"))?;

  // 2) Try simulate paste (macOS: Cmd+V requires Accessibility)
  let ok = std::panic::catch_unwind(|| {
    use enigo::{Enigo, Keyboard, Key, Direction, Settings};
    let mut enigo = Enigo::new(&Settings::default()).unwrap();

    #[cfg(target_os = "macos")]
    {
      // Cmd+V
      enigo.key(Key::Meta, Direction::Press).ok();
      enigo.key(Key::Unicode('v'), Direction::Click).ok();
      enigo.key(Key::Meta, Direction::Release).ok();
    }

    #[cfg(not(target_os = "macos"))]
    {
      // Ctrl+V
      enigo.key(Key::Control, Direction::Press).ok();
      enigo.key(Key::Unicode('v'), Direction::Click).ok();
      enigo.key(Key::Control, Direction::Release).ok();
    }
  })
  .is_ok();

  Ok(ok)
}

#[tauri::command]
fn get_settings(app: tauri::AppHandle) -> Result<AppSettings, String> {
  let config_dir = app
    .path()
    .app_config_dir()
    .map_err(|e| format!("config dir: {e}"))?;
  std::fs::create_dir_all(&config_dir).map_err(|e| format!("mkdir: {e}"))?;

  let settings_path = config_dir.join("settings.json");

  if !settings_path.exists() {
    return Ok(AppSettings::default());
  }

  let content = std::fs::read_to_string(&settings_path)
    .map_err(|e| format!("read settings: {e}"))?;

  let settings: AppSettings = serde_json::from_str(&content)
    .map_err(|e| format!("parse settings: {e}"))?;

  Ok(settings)
}

#[tauri::command]
fn save_settings(app: tauri::AppHandle, settings: AppSettings) -> Result<(), String> {
  let config_dir = app
    .path()
    .app_config_dir()
    .map_err(|e| format!("config dir: {e}"))?;
  std::fs::create_dir_all(&config_dir).map_err(|e| format!("mkdir: {e}"))?;

  let settings_path = config_dir.join("settings.json");
  let content = serde_json::to_string_pretty(&settings)
    .map_err(|e| format!("serialize settings: {e}"))?;

  std::fs::write(&settings_path, content)
    .map_err(|e| format!("write settings: {e}"))?;

  // Apply panel visibility setting
  #[cfg(desktop)]
  {
    if let Some(panel) = app.get_webview_window("panel") {
      if settings.panel_visible {
        let _ = panel.show();
      } else {
        let _ = panel.hide();
      }
    }
  }

  Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .plugin(tauri_plugin_clipboard_manager::init())
    .plugin(tauri_plugin_opener::init())
    .plugin(
      {
        #[cfg(desktop)]
        {
          use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut, ShortcutState};
          static EXPECTED_SHORTCUT: OnceLock<Shortcut> = OnceLock::new();
          // Use Ctrl+Shift+T (F1 may be reserved by macOS for brightness)
          EXPECTED_SHORTCUT.get_or_init(|| Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyT));

          tauri_plugin_global_shortcut::Builder::new()
            .with_handler(|app, shortcut, event| {
              eprintln!("Global shortcut triggered: {:?}, state: {:?}", shortcut, event.state());
              if let Some(expected) = EXPECTED_SHORTCUT.get() {
                if shortcut == expected && event.state() == ShortcutState::Pressed {
                  eprintln!("✓ Matching shortcut detected, emitting toggle event");
                  // Use app.emit to send to all windows, or window.emit for specific window
                  if let Some(w) = app.get_webview_window("panel") {
                    match w.emit("dictation-toggle", ()) {
                      Ok(_) => eprintln!("  → Event 'dictation-toggle' emitted successfully (window.emit)"),
                      Err(e) => eprintln!("  ✗ Failed to emit via window.emit: {:?}", e),
                    }
                  } else {
                    eprintln!("✗ Window 'panel' not found");
                  }
                } else {
                  eprintln!("✗ Shortcut mismatch or wrong state");
                }
              }
            })
            .build()
        }
        #[cfg(not(desktop))]
        {
          tauri_plugin_global_shortcut::Builder::new().build()
        }
      },
    )
    .setup(|app| {
      // ---------- Tray ----------
      #[cfg(desktop)]
      {
        use tauri::menu::{Menu, MenuItem};
        use tauri::tray::TrayIconBuilder;

        let toggle_i = MenuItem::with_id(app, "toggle", "Start/Stop Dictation", true, None::<&str>)?;
        let show_i   = MenuItem::with_id(app, "show", "Show Panel", true, None::<&str>)?;
        let settings_i = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
        let quit_i   = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
        let menu = Menu::with_items(app, &[&toggle_i, &show_i, &settings_i, &quit_i])?;

        let handle = app.handle().clone();
        TrayIconBuilder::new()
          .icon(app.default_window_icon().unwrap().clone())
          .menu(&menu)
          .show_menu_on_left_click(false)
          .on_menu_event(move |app, event| match event.id.as_ref() {
            "toggle" => {
              if let Some(w) = app.get_webview_window("panel") {
                let _ = w.emit("dictation-toggle", ());
              }
            }
            "show" => {
              if let Some(w) = app.get_webview_window("panel") {
                let _ = w.show();
                let _ = w.set_focus();
              }
            }
            "settings" => {
              if let Some(w) = app.get_webview_window("settings") {
                let _ = w.show();
                let _ = w.set_focus();
              }
            }
            "quit" => {
              app.exit(0);
            }
            _ => {}
          })
          .build(&handle)?;
      }

      // ---------- Panel default position and visibility ----------
      #[cfg(desktop)]
      {
        if let Some(panel) = app.get_webview_window("panel") {
          // Apply panel visibility from settings
          // Default to visible if settings don't exist or are corrupted
          let settings = match get_settings(app.handle().clone()) {
            Ok(s) => s,
            Err(e) => {
              eprintln!("Warning: Failed to load settings, using defaults: {}", e);
              AppSettings::default()
            }
          };

          eprintln!("Panel visibility setting: {}", settings.panel_visible);

          // Only hide if explicitly set to false, otherwise show
          if !settings.panel_visible {
            eprintln!("Hiding panel based on settings");
            if let Err(e) = panel.hide() {
              eprintln!("Warning: Failed to hide panel: {:?}", e);
            }
          } else {
            eprintln!("Showing panel (default or explicit setting)");
            if let Err(e) = panel.show() {
              eprintln!("Warning: Failed to show panel: {:?}", e);
            }
          }

          // Set default position
          let margin = 64.0;
          let monitor = panel
            .current_monitor()
            .ok()
            .flatten()
            .or_else(|| app.primary_monitor().ok().flatten());

          if let Some(monitor) = monitor {
            let scale_factor = monitor.scale_factor();
            let margin_px = (margin * scale_factor).round() as i32;
            let monitor_size = monitor.size();
            let window_size = panel.outer_size().unwrap_or(*monitor_size);

            let x = (monitor_size.width as i32 - window_size.width as i32 - margin_px).max(0);
            let y = (monitor_size.height as i32 - window_size.height as i32 - margin_px).max(0);
            let _ = panel.set_position(PhysicalPosition::new(x, y));
          }
        } else {
          eprintln!("Error: Panel window not found during setup - this should not happen!");
        }
      }

      // ---------- Global hotkey (Ctrl+Shift+T) ----------
      #[cfg(desktop)]
      {
        use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};

        let hk = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyT);
        match app.handle().global_shortcut().register(hk.clone()) {
          Ok(_) => {
            eprintln!("✓ Global shortcut Ctrl+Shift+T registered successfully");
          }
          Err(e) => {
            eprintln!("✗ Failed to register global shortcut Ctrl+Shift+T: {:?}", e);
            eprintln!("  Make sure the app has accessibility permissions on macOS");
          }
        }
      }

      Ok(())
    })
    .invoke_handler(tauri::generate_handler![
      greet,
      start_recording,
      stop_recording,
      openai_transcribe,
      google_transcribe,
      paste_text,
      get_settings,
      save_settings,
      list_input_devices,
      show_panel,
      hide_panel
    ])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}

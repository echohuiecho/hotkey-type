// src-tauri/src/lib.rs
use tauri::{Emitter, Manager};

#[cfg(desktop)]
use std::sync::OnceLock;

#[tauri::command]
fn greet(name: &str) -> String {
  format!("Hello, {}! You've been greeted from Rust!", name)
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
                  match app.emit("dictation-toggle", ()) {
                    Ok(_) => eprintln!("  → Event 'dictation-toggle' emitted successfully (app.emit)"),
                    Err(e) => {
                      eprintln!("  ✗ Failed to emit via app.emit: {:?}, trying window.emit", e);
                      if let Some(w) = app.get_webview_window("main") {
                        match w.emit("dictation-toggle", ()) {
                          Ok(_) => eprintln!("  → Event 'dictation-toggle' emitted successfully (window.emit)"),
                          Err(e2) => eprintln!("  ✗ Failed to emit via window.emit: {:?}", e2),
                        }
                      } else {
                        eprintln!("✗ Window 'main' not found");
                      }
                    }
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
        let quit_i   = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
        let menu = Menu::with_items(app, &[&toggle_i, &show_i, &quit_i])?;

        let handle = app.handle().clone();
        TrayIconBuilder::new()
          .icon(app.default_window_icon().unwrap().clone())
          .menu(&menu)
          .show_menu_on_left_click(false)
          .on_menu_event(move |app, event| match event.id.as_ref() {
            "toggle" => {
              if let Some(w) = app.get_webview_window("main") {
                let _ = w.emit("dictation-toggle", ());
              }
            }
            "show" => {
              if let Some(w) = app.get_webview_window("main") {
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
    .invoke_handler(tauri::generate_handler![greet])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}

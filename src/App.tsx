import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import Settings from "./Settings";
import "./App.css";

type Phase = "IDLE" | "RECORDING" | "TRANSCRIBING" | "PASTING" | "DONE" | "ERROR";
type View = "main" | "settings";

interface Settings {
  openai_api_key: string;
}

export default function App() {
  const [view, setView] = useState<View>("main");
  const [phase, setPhase] = useState<Phase>("IDLE");
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState<string>("");
  const [apiKey, setApiKey] = useState<string>("");
  const lastHandledRef = useRef<number>(0);
  const recordingRef = useRef<boolean>(false);
  const lastPathRef = useRef<string | null>(null);

  // Load settings on mount
  useEffect(() => {
    loadSettings();

    // Listen for open-settings event from tray menu
    const unlisten1 = listen("open-settings", () => {
      setView("settings");
      loadSettings();
    });

    // Listen for settings-updated event
    const unlisten2 = listen("settings-updated", () => {
      loadSettings();
    });

    return () => {
      unlisten1.then((fn) => fn());
      unlisten2.then((fn) => fn());
    };
  }, []);

  const loadSettings = async () => {
    try {
      const settings = await invoke<Settings>("get_settings");
      setApiKey(settings.openai_api_key || "");
    } catch (e) {
      console.error("Failed to load settings:", e);
    }
  };

  useEffect(() => {
    console.log("Setting up event listener for 'dictation-toggle'...");

    let unlistenFn: (() => void) | null = null;
    let cancelled = false;

    listen("dictation-toggle", async (event) => {
      console.log("✓ Toggle event received:", event);

      const now = Date.now();
      if (now - lastHandledRef.current < 150) {
        console.log("↪ Ignoring duplicate toggle within 150ms");
        return;
      }
      lastHandledRef.current = now;

      try {
        if (!recordingRef.current) {
          // Start recording
          recordingRef.current = true;
          setPhase("RECORDING");
          setError(null);
          setMessage("Recording...");

          const path = await invoke<string>("start_recording");
          lastPathRef.current = path;
          console.log("Recording started, path:", path);
        } else {
          // Stop recording
          recordingRef.current = false;
          setPhase("TRANSCRIBING");
          setMessage("Transcribing...");

          const stopped = await invoke<{ path: string; sample_rate: number; duration_ms: number }>("stop_recording");
          console.log("Recording stopped:", stopped);

          let keyToUse = apiKey?.trim() ?? "";
          if (!keyToUse) {
            // Reload settings in case the key was just saved
            const latest = await invoke<Settings>("get_settings");
            keyToUse = (latest.openai_api_key || "").trim();
            setApiKey(keyToUse);
          }
          if (!keyToUse) {
            throw new Error("Please set your OpenAI API key in Settings");
          }

          // Transcribe
          const { text } = await invoke<{ text: string }>("openai_transcribe", {
            audioPath: stopped.path,
            apiKey: keyToUse,
            model: "whisper-1",
          });

          console.log("Transcribed text:", text);

          if (!text || text.trim().length === 0) {
            setPhase("ERROR");
            setError("No text transcribed");
            setMessage("No text was transcribed from the audio");
            return;
          }

          // Paste
          setPhase("PASTING");
          setMessage("Pasting...");

          const pasted = await invoke<boolean>("paste_text", { text });

          if (pasted) {
            setPhase("DONE");
            setMessage(`Pasted: "${text}"`);
          } else {
            setPhase("DONE");
            setMessage(`Copied to clipboard (press ⌘V): "${text}"`);
          }

          // Auto return to IDLE after 2 seconds
          setTimeout(() => {
            setPhase("IDLE");
            setMessage("");
          }, 2000);
        }
      } catch (e) {
        recordingRef.current = false;
        setPhase("ERROR");
        const errorMsg = e instanceof Error ? e.message : String(e);
        setError(errorMsg);
        setMessage(`Error: ${errorMsg}`);
        console.error("Dictation error:", e);

        // Auto return to IDLE after 3 seconds on error
        setTimeout(() => {
          setPhase("IDLE");
          setError(null);
          setMessage("");
        }, 3000);
      }
    })
      .then((unlisten) => {
        if (cancelled) {
          unlisten();
          return;
        }
        unlistenFn = unlisten;
        console.log("✓ Listener for 'dictation-toggle' registered successfully");
      })
      .catch((err) => {
        console.error("✗ Failed to register listener for 'dictation-toggle':", err);
      });

    return () => {
      console.log("Cleaning up event listener...");
      cancelled = true;
      if (unlistenFn) {
        unlistenFn();
        console.log("Event listener unregistered");
      }
    };
  }, []);

  if (view === "settings") {
    return (
      <div>
        <div style={{ padding: "8px 16px", borderBottom: "1px solid #eee", display: "flex", justifyContent: "space-between", alignItems: "center" }}>
          <div style={{ fontSize: 14, fontWeight: 500 }}>Settings</div>
          <button
            onClick={() => {
              setView("main");
              loadSettings();
            }}
            style={{
              padding: "4px 12px",
              fontSize: 12,
              backgroundColor: "transparent",
              border: "1px solid #ddd",
              borderRadius: 4,
              cursor: "pointer",
            }}
          >
            Back
          </button>
        </div>
        <Settings />
      </div>
    );
  }

  return (
    <div style={{ padding: 16, fontFamily: "system-ui" }}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 8 }}>
        <div style={{ fontSize: 14, opacity: 0.7 }}>Dictation Panel</div>
        <button
          onClick={() => {
            setView("settings");
            loadSettings();
          }}
          style={{
            padding: "4px 8px",
            fontSize: 11,
            backgroundColor: "transparent",
            border: "1px solid #ddd",
            borderRadius: 4,
            cursor: "pointer",
            opacity: 0.7,
          }}
          title="Open Settings"
        >
          ⚙️
        </button>
      </div>
      <div style={{ fontSize: 24, marginTop: 8, fontWeight: "bold" }}>{phase}</div>
      {message && (
        <div style={{ marginTop: 8, fontSize: 14, color: phase === "ERROR" ? "#ff4444" : "#666" }}>
          {message}
        </div>
      )}
      {error && (
        <div style={{ marginTop: 8, fontSize: 12, color: "#ff4444" }}>
          {error}
        </div>
      )}
      <div style={{ marginTop: 8, fontSize: 12, opacity: 0.6 }}>
        Press Ctrl+Shift+T to toggle
      </div>
      {!apiKey || apiKey.trim().length === 0 ? (
        <div style={{ marginTop: 8, fontSize: 11, color: "#ff8800" }}>
          ⚠️ Please set your OpenAI API key in Settings
        </div>
      ) : null}
    </div>
  );
}

import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import Settings from "./Settings";
import "./App.css";

type Phase = "IDLE" | "RECORDING" | "TRANSCRIBING" | "PASTING" | "DONE" | "ERROR";

interface Settings {
  provider: string;
  openai_api_key: string;
  google_api_key: string;
  google_language: string;
  input_device_name: string;
  panel_visible?: boolean;
}

export default function App() {
  const [windowLabel, setWindowLabel] = useState<string | null>(null);
  const [phase, setPhase] = useState<Phase>("IDLE");
  const [message, setMessage] = useState<string>("");
  const [provider, setProvider] = useState<"openai" | "google">("openai");
  const [openaiApiKey, setOpenaiApiKey] = useState<string>("");
  const [googleApiKey, setGoogleApiKey] = useState<string>("");
  const [googleLanguage, setGoogleLanguage] = useState<string>("en-US");
  const lastHandledRef = useRef<number>(0);
  const recordingRef = useRef<boolean>(false);
  const lastPathRef = useRef<string | null>(null);

  // Get window label on mount
  useEffect(() => {
    const initWindow = async () => {
      try {
        const window = getCurrentWindow();
        const label = window.label;
        setWindowLabel(label);
      } catch (e) {
        console.error("Failed to get window label:", e);
      }
    };
    initWindow();
  }, []);

  useEffect(() => {
    if (windowLabel !== "panel") {
      return;
    }
    // Set data attribute for CSS targeting
    document.documentElement.setAttribute("data-panel-window", "true");
    document.documentElement.style.overflow = "hidden";
    document.documentElement.style.background = "transparent";
    document.body.style.overflow = "hidden";
    document.body.style.margin = "0";
    document.body.style.background = "transparent";
    document.body.style.padding = "0";

    // Also ensure root element is transparent
    const root = document.getElementById("root");
    if (root) {
      root.style.background = "transparent";
      root.style.margin = "0";
      root.style.padding = "0";
    }
  }, [windowLabel]);

  // Load settings on mount
  useEffect(() => {
    loadSettings();

    // Listen for settings-updated event
    const unlisten = listen("settings-updated", () => {
      loadSettings();
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const loadSettings = async () => {
    try {
      const settings = await invoke<Settings>("get_settings");
      setProvider(settings.provider === "google" ? "google" : "openai");
      setOpenaiApiKey(settings.openai_api_key || "");
      setGoogleApiKey(settings.google_api_key || "");
      setGoogleLanguage(settings.google_language || "en-US");
    } catch (e) {
      console.error("Failed to load settings:", e);
    }
  };

  // Only set up dictation toggle listener for panel window
  useEffect(() => {
    if (windowLabel !== "panel") {
      return;
    }

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

          let providerToUse = provider;
          let openaiKeyToUse = openaiApiKey?.trim() ?? "";
          let googleKeyToUse = googleApiKey?.trim() ?? "";
          let googleLanguageToUse = googleLanguage?.trim() ?? "";
          const needsRefresh =
            !providerToUse ||
            (providerToUse === "openai" && !openaiKeyToUse) ||
            (providerToUse === "google" && !googleKeyToUse);

          if (needsRefresh) {
            // Reload settings in case the key was just saved
            const latest = await invoke<Settings>("get_settings");
            providerToUse = latest.provider === "google" ? "google" : "openai";
            openaiKeyToUse = (latest.openai_api_key || "").trim();
            googleKeyToUse = (latest.google_api_key || "").trim();
            googleLanguageToUse = (latest.google_language || "").trim();
            setProvider(providerToUse);
            setOpenaiApiKey(openaiKeyToUse);
            setGoogleApiKey(googleKeyToUse);
            setGoogleLanguage(googleLanguageToUse || "en-US");
          }
          if (providerToUse === "openai" && !openaiKeyToUse) {
            throw new Error("Please set your OpenAI API key in Settings");
          }
          if (providerToUse === "google" && !googleKeyToUse) {
            throw new Error("Please set your Google API key in Settings");
          }
          if (providerToUse === "google" && !googleLanguageToUse) {
            googleLanguageToUse = "en-US";
          }

          // Transcribe
          const { text } =
            providerToUse === "google"
              ? await invoke<{ text: string }>("google_transcribe", {
                  audioPath: stopped.path,
                  apiKey: googleKeyToUse,
                  language: googleLanguageToUse,
                  model: "default",
                  enableAutomaticPunctuation: true,
                })
              : await invoke<{ text: string }>("openai_transcribe", {
                  audioPath: stopped.path,
                  apiKey: openaiKeyToUse,
                  model: "whisper-1",
                });

          console.log("Transcribed text:", text);

          if (!text || text.trim().length === 0) {
            setPhase("ERROR");
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
        setMessage(`Error: ${errorMsg}`);
        console.error("Dictation error:", e);

        // Auto return to IDLE after 3 seconds on error
        setTimeout(() => {
          setPhase("IDLE");
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
  }, [windowLabel]);

  // Render Settings window
  if (windowLabel === "settings") {
    return (
      <div style={{ fontFamily: "system-ui", height: "100vh", overflow: "auto" }}>
        <div
          style={{
            padding: "8px 16px",
            borderBottom: "1px solid #eee",
            display: "flex",
            justifyContent: "space-between",
            alignItems: "center",
          }}
        >
          <div style={{ fontSize: 14, fontWeight: 500 }}>Settings</div>
        </div>
        <Settings />
      </div>
    );
  }

  // Render Panel window (circular floating panel)
  if (windowLabel === "panel") {
    const getStatusColor = () => {
      switch (phase) {
        case "RECORDING":
          return "#ff4444";
        case "ERROR":
          return "#ff8800";
        case "TRANSCRIBING":
        case "PASTING":
          return "#007AFF";
        case "DONE":
          return "#28a745";
        default:
          return "#666";
      }
    };

    const startDrag = async (e: React.PointerEvent | React.MouseEvent) => {
      // Only left click / primary pointer
      if ("button" in e && typeof e.button === "number" && e.button !== 0) return;

      e.preventDefault();
      e.stopPropagation();
      try {
        await getCurrentWindow().startDragging();
      } catch (err) {
        console.error("Failed to start dragging:", err);
      }
    };

    return (
      <div
        data-tauri-drag-region
        onPointerDown={startDrag}
        onMouseDown={startDrag}
        style={{
          width: "100vw",
          height: "100vh",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          cursor: "grab",
          overflow: "hidden",
          background: "transparent",
          margin: 0,
          padding: 0,
        }}
      >
        <div
          style={{
            width: 64,
            height: 64,
            borderRadius: "50%",
            backgroundColor: phase === "RECORDING" ? "rgba(255, 68, 68, 0.25)" : "rgba(0, 0, 0, 0.08)",
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            fontFamily: "system-ui",
            position: "relative",
            transition: "all 0.2s ease",
            userSelect: "none",
            pointerEvents: "none",
          }}
        >
          {phase === "RECORDING" && (
            <div
              style={{
                width: 12,
                height: 12,
                borderRadius: "50%",
                backgroundColor: "#ff4444",
                animation: "pulse 1.5s ease-in-out infinite",
              }}
            />
          )}
          {phase === "IDLE" && (
            <div style={{ fontSize: 20, opacity: 0.6 }}>🎤</div>
          )}
          {phase !== "IDLE" && phase !== "RECORDING" && (
            <div style={{ fontSize: 12, color: getStatusColor(), fontWeight: "bold" }}>
              {phase === "TRANSCRIBING" ? "⏳" : phase === "PASTING" ? "📋" : phase === "DONE" ? "✓" : "⚠"}
            </div>
          )}
          {message && (
            <div
              style={{
                position: "absolute",
                top: -30,
                left: "50%",
                transform: "translateX(-50%)",
                fontSize: 10,
                color: phase === "ERROR" ? "#ff4444" : "#666",
                whiteSpace: "nowrap",
                backgroundColor: "rgba(255, 255, 255, 0.95)",
                padding: "2px 6px",
                borderRadius: 4,
                boxShadow: "0 2px 4px rgba(0,0,0,0.1)",
                pointerEvents: "none",
              }}
            >
              {message.length > 20 ? message.substring(0, 20) + "..." : message}
            </div>
          )}
        </div>
      </div>
    );
  }

  // Loading state while determining window label
  return (
    <div style={{ padding: 16, fontFamily: "system-ui" }}>
      <div>Loading...</div>
    </div>
  );
}

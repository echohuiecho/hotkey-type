import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import "./App.css";

type Phase = "IDLE" | "RECORDING" | "PROCESSING";

export default function App() {
  const [phase, setPhase] = useState<Phase>("IDLE");
  const lastHandledRef = useRef<number>(0);

  useEffect(() => {
    console.log("Setting up event listener for 'dictation-toggle'...");

    let unlistenFn: (() => void) | null = null;
    let timeoutId: ReturnType<typeof setTimeout> | null = null;
    let cancelled = false;

    listen("dictation-toggle", (event) => {
      console.log("✓ Toggle event received:", event);

      const now = Date.now();
      if (now - lastHandledRef.current < 150) {
        console.log("↪ Ignoring duplicate toggle within 150ms");
        return;
      }
      lastHandledRef.current = now;

      // Clear any pending timeout
      if (timeoutId) {
        clearTimeout(timeoutId);
        timeoutId = null;
      }

      setPhase((p) => {
        let newPhase: Phase;
        if (p === "IDLE") {
          // Start recording
          newPhase = "RECORDING";
        } else if (p === "RECORDING") {
          // Stop recording and process
          newPhase = "PROCESSING";
          // Auto return to IDLE after processing
          timeoutId = setTimeout(() => {
            setPhase("IDLE");
            console.log("Phase: PROCESSING → IDLE");
          }, 600);
        } else {
          // If already processing, just reset to IDLE
          newPhase = "IDLE";
        }
        console.log(`Phase: ${p} → ${newPhase}`);
        return newPhase;
      });
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
      if (timeoutId) {
        clearTimeout(timeoutId);
      }
      if (unlistenFn) {
        unlistenFn();
        console.log("Event listener unregistered");
      }
    };
  }, []);

  return (
    <div style={{ padding: 16, fontFamily: "system-ui" }}>
      <div style={{ fontSize: 14, opacity: 0.7 }}>Dictation Panel</div>
      <div style={{ fontSize: 24, marginTop: 8 }}>{phase}</div>
      <div style={{ marginTop: 8, fontSize: 12, opacity: 0.6 }}>
        Press Ctrl+Shift+T to toggle
      </div>
    </div>
  );
}

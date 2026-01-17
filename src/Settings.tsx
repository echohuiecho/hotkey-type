import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

interface Settings {
  openai_api_key: string;
}

export default function Settings() {
  const [settings, setSettings] = useState<Settings>({ openai_api_key: "" });
  const [saved, setSaved] = useState(false);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    // Load settings on mount
    loadSettings();

    // Listen for open-settings event
    const unlisten = listen("open-settings", () => {
      loadSettings();
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const loadSettings = async () => {
    try {
      setLoading(true);
      const loaded = await invoke<Settings>("get_settings");
      setSettings(loaded);
    } catch (e) {
      console.error("Failed to load settings:", e);
    } finally {
      setLoading(false);
    }
  };

  const handleSave = async () => {
    try {
      await invoke("save_settings", { settings });
      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
      // Emit event to notify main view to reload settings
      const { emit } = await import("@tauri-apps/api/event");
      await emit("settings-updated", {});
    } catch (e) {
      console.error("Failed to save settings:", e);
      alert(`Failed to save settings: ${e}`);
    }
  };

  if (loading) {
    return (
      <div style={{ padding: 16, fontFamily: "system-ui" }}>
        <div>Loading settings...</div>
      </div>
    );
  }

  return (
    <div style={{ padding: 16, fontFamily: "system-ui", maxWidth: 500 }}>
      <h2 style={{ marginTop: 0, fontSize: 20, fontWeight: "bold" }}>Settings</h2>

      <div style={{ marginTop: 16 }}>
        <label style={{ display: "block", marginBottom: 8, fontSize: 14, fontWeight: 500 }}>
          OpenAI API Key
        </label>
        <input
          type="password"
          value={settings.openai_api_key}
          onChange={(e) => setSettings({ ...settings, openai_api_key: e.target.value })}
          placeholder="sk-..."
          style={{
            width: "100%",
            padding: "8px 12px",
            fontSize: 14,
            border: "1px solid #ddd",
            borderRadius: 4,
            fontFamily: "monospace",
            boxSizing: "border-box",
          }}
        />
        <div style={{ marginTop: 4, fontSize: 12, color: "#666" }}>
          Your API key is stored locally and never shared.
        </div>
      </div>

      <div style={{ marginTop: 24, display: "flex", gap: 8, alignItems: "center" }}>
        <button
          onClick={handleSave}
          style={{
            padding: "8px 16px",
            fontSize: 14,
            backgroundColor: "#007AFF",
            color: "white",
            border: "none",
            borderRadius: 4,
            cursor: "pointer",
            fontWeight: 500,
          }}
          onMouseOver={(e) => {
            e.currentTarget.style.backgroundColor = "#0051D5";
          }}
          onMouseOut={(e) => {
            e.currentTarget.style.backgroundColor = "#007AFF";
          }}
        >
          Save
        </button>
        {saved && (
          <span style={{ fontSize: 14, color: "#28a745" }}>✓ Saved</span>
        )}
      </div>

      <div style={{ marginTop: 24, padding: 12, backgroundColor: "#f5f5f5", borderRadius: 4, fontSize: 12 }}>
        <strong>Note:</strong> You can get your API key from{" "}
        <a
          href="https://platform.openai.com/api-keys"
          target="_blank"
          rel="noopener noreferrer"
          style={{ color: "#007AFF" }}
        >
          OpenAI Platform
        </a>
      </div>
    </div>
  );
}

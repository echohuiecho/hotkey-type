import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

interface Settings {
  provider: string;
  openai_api_key: string;
  google_api_key: string;
  google_language: string;
}

export default function Settings() {
  const [settings, setSettings] = useState<Settings>({
    provider: "openai",
    openai_api_key: "",
    google_api_key: "",
    google_language: "en-US",
  });
  const [saved, setSaved] = useState(false);
  const [loading, setLoading] = useState(true);
  const googleLanguageOptions = [
    { label: "English (United States) — en-US", value: "en-US" },
    { label: "English (United Kingdom) — en-GB", value: "en-GB" },
    { label: "Cantonese (Traditional, Hong Kong) — yue-Hant-HK", value: "yue-Hant-HK" },
    { label: "Mandarin Chinese (Simplified) — zh", value: "zh" },
    { label: "Mandarin Chinese (China) — zh-CN", value: "zh-CN" },
    { label: "Mandarin Chinese (Taiwan) — zh-TW", value: "zh-TW" },
    { label: "Japanese — ja-JP", value: "ja-JP" },
    { label: "Korean — ko-KR", value: "ko-KR" },
    { label: "Spanish (Spain) — es-ES", value: "es-ES" },
    { label: "Spanish (United States) — es-US", value: "es-US" },
    { label: "French (France) — fr-FR", value: "fr-FR" },
    { label: "German — de-DE", value: "de-DE" },
  ];

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
      setSettings({
        provider: loaded.provider || "openai",
        openai_api_key: loaded.openai_api_key || "",
        google_api_key: loaded.google_api_key || "",
        google_language: loaded.google_language || "en-US",
      });
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
          Transcription Provider
        </label>
        <div style={{ display: "flex", gap: 16, fontSize: 14 }}>
          <label style={{ display: "flex", alignItems: "center", gap: 6 }}>
            <input
              type="radio"
              name="provider"
              checked={settings.provider === "openai"}
              onChange={() => setSettings({ ...settings, provider: "openai" })}
            />
            OpenAI
          </label>
          <label style={{ display: "flex", alignItems: "center", gap: 6 }}>
            <input
              type="radio"
              name="provider"
              checked={settings.provider === "google"}
              onChange={() => setSettings({ ...settings, provider: "google" })}
            />
            Google
          </label>
        </div>
      </div>

      {settings.provider === "openai" && (
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
      )}

      {settings.provider === "google" && (
        <div style={{ marginTop: 16 }}>
          <label style={{ display: "block", marginBottom: 8, fontSize: 14, fontWeight: 500 }}>
            Google API Key
          </label>
          <input
            type="password"
            value={settings.google_api_key}
            onChange={(e) => setSettings({ ...settings, google_api_key: e.target.value })}
            placeholder="AIza..."
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
          <div style={{ marginTop: 12 }}>
            <label style={{ display: "block", marginBottom: 8, fontSize: 14, fontWeight: 500 }}>
              Google Language Code
            </label>
            <select
              value={settings.google_language}
              onChange={(e) => setSettings({ ...settings, google_language: e.target.value })}
              style={{
                width: "100%",
                padding: "8px 12px",
                fontSize: 14,
                border: "1px solid #ddd",
                borderRadius: 4,
                fontFamily: "monospace",
                boxSizing: "border-box",
              }}
            >
              {googleLanguageOptions.map((option) => (
                <option key={option.value} value={option.value}>
                  {option.label}
                </option>
              ))}
            </select>
            <div style={{ marginTop: 4, fontSize: 12, color: "#666" }}>
              Uses Google language codes.
            </div>
          </div>
        </div>
      )}

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
          href={
            settings.provider === "google"
              ? "https://console.cloud.google.com/apis/credentials"
              : "https://platform.openai.com/api-keys"
          }
          target="_blank"
          rel="noopener noreferrer"
          style={{ color: "#007AFF" }}
        >
          {settings.provider === "google" ? "Google Cloud Console" : "OpenAI Platform"}
        </a>
      </div>
    </div>
  );
}

import { Component, createSignal, onMount } from "solid-js";
import { getAppVersion, getLicenses } from "../lib/tauri-api";

interface AboutDialogProps {
  onClose: () => void;
}

const AboutDialog: Component<AboutDialogProps> = (props) => {
  const [version, setVersion] = createSignal("...");
  const [licenses, setLicenses] = createSignal("Loading...");

  onMount(async () => {
    try {
      setVersion(await getAppVersion());
    } catch {
      setVersion("unknown");
    }
    try {
      setLicenses(await getLicenses());
    } catch {
      setLicenses("Failed to load licenses");
    }
  });

  return (
    <div
      style={{
        position: "fixed",
        top: "0",
        left: "0",
        right: "0",
        bottom: "0",
        background: "rgba(0,0,0,0.7)",
        display: "flex",
        "align-items": "center",
        "justify-content": "center",
        "z-index": "200",
      }}
      onClick={(e) => {
        if (e.target === e.currentTarget) props.onClose();
      }}
    >
      <div style={{
        background: "var(--bg-primary)",
        "border-radius": "12px",
        padding: "24px",
        "max-width": "500px",
        width: "90%",
        "max-height": "80vh",
        overflow: "auto",
      }}>
        <div style={{
          display: "flex",
          "justify-content": "space-between",
          "align-items": "center",
          "margin-bottom": "16px",
        }}>
          <h2 style={{ margin: "0" }}>About Simple Image Gen</h2>
          <button
            onClick={props.onClose}
            style={{
              background: "none",
              border: "none",
              color: "var(--text-secondary)",
              "font-size": "24px",
              cursor: "pointer",
              "line-height": "1",
            }}
          >
            &#x2715;
          </button>
        </div>

        <div style={{ "margin-bottom": "16px" }}>
          <p>Version {version()}</p>
          <p style={{
            "font-size": "13px",
            color: "var(--text-secondary)",
            "margin-top": "8px",
          }}>
            Dead-simple local AI image generation. Type a prompt, get an image.
          </p>
        </div>

        <div style={{
          padding: "12px",
          background: "var(--bg-secondary)",
          "border-radius": "var(--radius)",
          "margin-bottom": "16px",
        }}>
          <div style={{ "font-weight": "bold", "margin-bottom": "8px", "font-size": "14px" }}>
            Credits
          </div>
          <div style={{
            "font-size": "13px",
            color: "var(--text-secondary)",
            "line-height": "1.6",
          }}>
            <div>
              Powered by <strong>stable-diffusion.cpp</strong>
            </div>
            <div>Copyright (c) 2023 leejet — MIT License</div>
            <div style={{ "margin-top": "8px" }}>
              Built with <strong>Tauri v2</strong>
            </div>
            <div>Apache-2.0 / MIT License</div>
          </div>
        </div>

        <div>
          <div style={{
            "font-weight": "bold",
            "margin-bottom": "8px",
            "font-size": "14px",
          }}>
            Third-Party Licenses
          </div>
          <pre style={{
            "font-size": "11px",
            color: "var(--text-secondary)",
            "white-space": "pre-wrap",
            "max-height": "200px",
            overflow: "auto",
            padding: "8px",
            background: "var(--bg-secondary)",
            "border-radius": "4px",
            margin: "0",
          }}>
            {licenses()}
          </pre>
        </div>

        <div style={{
          "margin-top": "16px",
          "text-align": "center",
          "font-size": "12px",
          color: "var(--text-secondary)",
        }}>
          MIT License | Beargle Industries 2026
        </div>
      </div>
    </div>
  );
};

export default AboutDialog;

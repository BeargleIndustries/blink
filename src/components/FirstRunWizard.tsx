import { Component, For, Show } from "solid-js";
import type { ModelInfo, SystemInfo } from "../lib/types";

interface FirstRunWizardProps {
  models: ModelInfo[];
  systemInfo: SystemInfo | null;
  downloading: string | null;
  onDownload: (modelId: string) => void;
  onSkip: () => void;
}

const FirstRunWizard: Component<FirstRunWizardProps> = (props) => {
  const recommendedModel = () => {
    if (props.systemInfo?.compiled_backend === "cpu") {
      return "sd15-q5";
    }
    return "sd15-q5";
  };

  return (
    <div style={{
      position: "fixed",
      top: "0",
      left: "0",
      right: "0",
      bottom: "0",
      background: "rgba(0,0,0,0.8)",
      display: "flex",
      "align-items": "center",
      "justify-content": "center",
      "z-index": "200",
    }}>
      <div style={{
        background: "var(--bg-secondary)",
        border: "1px solid var(--border)",
        "border-radius": "12px",
        padding: "32px",
        "max-width": "500px",
        width: "90%",
        "text-align": "center",
      }}>
        <h1 style={{ "margin-bottom": "4px", "font-size": "24px", color: "var(--text-primary)" }}>
          Welcome to Blink
        </h1>
        <p style={{
          "font-size": "11px",
          color: "var(--text-muted)",
          margin: "0 0 16px 0",
        }}>
          A Beargle Industries project
        </p>
        <p style={{ color: "var(--text-secondary)", "margin-bottom": "24px", "font-size": "14px" }}>
          Download a model to get started. <strong style={{ color: "var(--text-primary)" }}>Stable Diffusion 1.5</strong> is fast and works on most hardware.
        </p>

        <Show when={props.systemInfo}>
          <div style={{
            padding: "8px 12px",
            background: "var(--bg-secondary)",
            "border-radius": "var(--radius)",
            "margin-bottom": "16px",
            "font-size": "13px",
          }}>
            Backend:{" "}
            <strong>{props.systemInfo!.compiled_backend.toUpperCase()}</strong>
            {" | "}
            {props.systemInfo!.os} ({props.systemInfo!.arch})
          </div>
        </Show>

        <div style={{ display: "flex", "flex-direction": "column", gap: "8px" }}>
          <For each={props.models}>
            {(model) => {
              const isRecommended = () => model.id === recommendedModel();
              const isDownloading = () => props.downloading === model.id;
              return (
                <button
                  onClick={() => props.onDownload(model.id)}
                  disabled={!!props.downloading}
                  style={{
                    padding: "12px 16px",
                    background: isDownloading()
                      ? "var(--bg-tertiary)"
                      : isRecommended()
                        ? "var(--accent)"
                        : "var(--bg-secondary)",
                    border: isRecommended() && !isDownloading()
                      ? "none"
                      : "1px solid var(--border)",
                    "border-radius": "var(--radius)",
                    color: isDownloading()
                      ? "var(--text-secondary)"
                      : isRecommended() ? "white" : "var(--text-primary)",
                    cursor: props.downloading ? "not-allowed" : "pointer",
                    "text-align": "left",
                    "font-size": "14px",
                    opacity: props.downloading && !isDownloading() ? "0.5" : "1",
                  }}
                >
                  <div style={{ "font-weight": "bold" }}>
                    {isDownloading() ? `Downloading ${model.name}...` : model.name}
                    {!isDownloading() && isRecommended() ? " (Recommended)" : ""}
                  </div>
                  <div style={{
                    "font-size": "12px",
                    "margin-top": "4px",
                    opacity: "0.8",
                  }}>
                    {isDownloading()
                      ? "This may take a few minutes"
                      : `${(model.size_bytes / 1_000_000_000).toFixed(1)} GB | ~${(model.vram_mb / 1000).toFixed(1)} GB VRAM`}
                  </div>
                </button>
              );
            }}
          </For>
        </div>

        <button
          onClick={props.onSkip}
          style={{
            "margin-top": "16px",
            background: "none",
            border: "none",
            color: "var(--text-secondary)",
            cursor: "pointer",
            "font-size": "13px",
          }}
        >
          Skip for now
        </button>
      </div>
    </div>
  );
};

export default FirstRunWizard;

import { Component, For, Show } from "solid-js";
import type { ModelInfo, SystemInfo } from "../lib/types";

interface FirstRunWizardProps {
  models: ModelInfo[];
  systemInfo: SystemInfo | null;
  onDownload: (modelId: string) => void;
  onSkip: () => void;
}

const FirstRunWizard: Component<FirstRunWizardProps> = (props) => {
  const recommendedModel = () => {
    if (props.systemInfo?.compiled_backend === "cpu") {
      return "sd15-q8";
    }
    return "sd15-q8";
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
        background: "var(--bg-primary)",
        "border-radius": "12px",
        padding: "32px",
        "max-width": "500px",
        width: "90%",
        "text-align": "center",
      }}>
        <h1 style={{ "margin-bottom": "8px", "font-size": "24px" }}>
          Welcome to Simple Image Gen
        </h1>
        <p style={{ color: "var(--text-secondary)", "margin-bottom": "24px" }}>
          To get started, download an AI model. We recommend starting with{" "}
          <strong>Stable Diffusion 1.5</strong> — it's fast and works on most
          hardware.
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
              return (
                <button
                  onClick={() => props.onDownload(model.id)}
                  style={{
                    padding: "12px 16px",
                    background: isRecommended()
                      ? "var(--accent)"
                      : "var(--bg-secondary)",
                    border: isRecommended()
                      ? "none"
                      : "1px solid var(--border)",
                    "border-radius": "var(--radius)",
                    color: isRecommended() ? "white" : "var(--text-primary)",
                    cursor: "pointer",
                    "text-align": "left",
                    "font-size": "14px",
                  }}
                >
                  <div style={{ "font-weight": "bold" }}>
                    {model.name}
                    {isRecommended() ? " (Recommended)" : ""}
                  </div>
                  <div style={{
                    "font-size": "12px",
                    "margin-top": "4px",
                    opacity: "0.8",
                  }}>
                    {(model.size_bytes / 1_000_000_000).toFixed(1)} GB | ~
                    {(model.vram_mb / 1000).toFixed(1)} GB VRAM
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

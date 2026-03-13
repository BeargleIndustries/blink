import { Component, For } from "solid-js";
import type { ModelInfo } from "../lib/types";
import ModelCard from "./ModelCard";

interface ModelBrowserProps {
  models: ModelInfo[];
  onDownload: (modelId: string) => void;
  onDelete: (modelId: string) => void;
  onSelect: (modelId: string) => void;
  onClose: () => void;
}

const ModelBrowser: Component<ModelBrowserProps> = (props) => {
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
        "z-index": "100",
      }}
      onClick={(e) => {
        if (e.target === e.currentTarget) props.onClose();
      }}
    >
      <div style={{
        background: "var(--bg-primary)",
        "border-radius": "12px",
        padding: "24px",
        width: "90%",
        "max-width": "800px",
        "max-height": "80vh",
        overflow: "auto",
      }}>
        <div style={{
          display: "flex",
          "justify-content": "space-between",
          "align-items": "center",
          "margin-bottom": "16px",
        }}>
          <h2 style={{ margin: "0", "font-size": "20px" }}>Model Browser</h2>
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

        <div style={{
          display: "grid",
          "grid-template-columns": "repeat(auto-fill, minmax(220px, 1fr))",
          gap: "16px",
        }}>
          <For each={props.models}>
            {(model) => (
              <ModelCard
                model={model}
                onDownload={() => props.onDownload(model.id)}
                onDelete={() => props.onDelete(model.id)}
                onSelect={() => props.onSelect(model.id)}
              />
            )}
          </For>
        </div>
      </div>
    </div>
  );
};

export default ModelBrowser;

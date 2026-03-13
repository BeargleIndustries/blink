import { Component, Show } from "solid-js";
import type { ModelInfo } from "../lib/types";
import LicenseInfo from "./LicenseInfo";

interface ModelCardProps {
  model: ModelInfo;
  onDownload: () => void;
  onDelete: () => void;
  onSelect: () => void;
}

const ModelCard: Component<ModelCardProps> = (props) => {
  const sizeGB = () => (props.model.size_bytes / 1_000_000_000).toFixed(1);
  const vramGB = () => (props.model.vram_mb / 1000).toFixed(1);

  return (
    <div style={{
      background: "var(--bg-secondary)",
      "border-radius": "var(--radius)",
      padding: "16px",
      border: props.model.active
        ? "2px solid var(--accent)"
        : "1px solid var(--border)",
      display: "flex",
      "flex-direction": "column",
      gap: "8px",
    }}>
      <div style={{ "font-weight": "bold", "font-size": "15px" }}>
        {props.model.name}
      </div>
      <div style={{
        "font-size": "12px",
        color: "var(--text-secondary)",
        "line-height": "1.4",
      }}>
        {props.model.description}
      </div>

      <div style={{
        display: "flex",
        gap: "8px",
        "flex-wrap": "wrap",
        "font-size": "11px",
      }}>
        <span style={{
          padding: "2px 6px",
          background: "var(--bg-tertiary)",
          "border-radius": "4px",
        }}>
          {sizeGB()} GB
        </span>
        <span style={{
          padding: "2px 6px",
          background: "var(--bg-tertiary)",
          "border-radius": "4px",
        }}>
          ~{vramGB()} GB VRAM
        </span>
        <span style={{
          padding: "2px 6px",
          background: "var(--bg-tertiary)",
          "border-radius": "4px",
        }}>
          {props.model.architecture.toUpperCase()}
        </span>
      </div>

      <LicenseInfo
        name={props.model.license_name}
        url={props.model.license_url}
        commercial={props.model.commercial}
      />

      <Show
        when={props.model.downloaded}
        fallback={
          <button
            onClick={props.onDownload}
            style={{
              padding: "8px",
              background: "var(--accent)",
              border: "none",
              "border-radius": "var(--radius)",
              color: "white",
              cursor: "pointer",
              "font-size": "13px",
            }}
          >
            Download
          </button>
        }
      >
        <div style={{ display: "flex", gap: "8px" }}>
          <button
            onClick={props.onSelect}
            style={{
              flex: 1,
              padding: "8px",
              background: props.model.active
                ? "var(--success)"
                : "var(--bg-tertiary)",
              border: "1px solid var(--border)",
              "border-radius": "var(--radius)",
              color: "var(--text-primary)",
              cursor: "pointer",
              "font-size": "13px",
            }}
          >
            {props.model.active ? "Active" : "Use"}
          </button>
          <button
            onClick={props.onDelete}
            style={{
              padding: "8px 12px",
              background: "none",
              border: "1px solid var(--border)",
              "border-radius": "var(--radius)",
              color: "var(--error)",
              cursor: "pointer",
              "font-size": "13px",
            }}
          >
            Delete
          </button>
        </div>
      </Show>
    </div>
  );
};

export default ModelCard;

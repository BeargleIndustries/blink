import { Component, For, Show } from "solid-js";
import type { ModelInfo } from "../lib/types";
import LicenseInfo from "./LicenseInfo";

interface ModelCardProps {
  model: ModelInfo;
  downloading: boolean;
  downloadProgress?: { fileRole: string; fileIndex: number; totalFiles: number } | null;
  onDownload: () => void;
  onDelete: () => void;
  onSelect: () => void;
  vramTotalMb?: number | null;
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
          "border-radius": "var(--radius-pill)",
        }}>
          {sizeGB()} GB
        </span>
        <span style={{
          padding: "2px 6px",
          background: props.vramTotalMb && props.model.vram_mb > props.vramTotalMb
            ? "rgba(239, 68, 68, 0.1)"
            : "var(--bg-tertiary)",
          color: props.vramTotalMb && props.model.vram_mb > props.vramTotalMb
            ? "var(--error)"
            : "inherit",
          border: props.vramTotalMb && props.model.vram_mb > props.vramTotalMb
            ? "1px solid rgba(239, 68, 68, 0.2)"
            : "1px solid transparent",
          "border-radius": "var(--radius-pill)",
        }}
          title={props.vramTotalMb && props.model.vram_mb > props.vramTotalMb
            ? `Needs ~${vramGB()} GB but you have ${(props.vramTotalMb / 1000).toFixed(1)} GB — may be slow or fail`
            : `Estimated VRAM usage`}
        >
          ~{vramGB()} GB VRAM
          {props.vramTotalMb && props.model.vram_mb > props.vramTotalMb ? " !" : ""}
        </span>
        <span style={{
          padding: "2px 6px",
          background: "var(--bg-tertiary)",
          "border-radius": "var(--radius-pill)",
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
          <div style={{ display: "flex", "flex-direction": "column", gap: "6px" }}>
            <button
              onClick={props.onDownload}
              disabled={props.downloading}
              style={{
                padding: "8px",
                background: props.downloading ? "var(--bg-tertiary)" : "var(--accent)",
                border: "none",
                "border-radius": "var(--radius)",
                color: props.downloading ? "var(--text-secondary)" : "white",
                cursor: props.downloading ? "not-allowed" : "pointer",
                "font-size": "13px",
              }}
            >
              {props.downloading && props.downloadProgress && props.downloadProgress.totalFiles > 1
                ? `Downloading ${props.downloadProgress.fileIndex + 1}/${props.downloadProgress.totalFiles}: ${props.downloadProgress.fileRole}`
                : props.downloading
                ? "Downloading..."
                : "Download"}
            </button>
            <Show when={props.downloading && props.downloadProgress && props.downloadProgress.totalFiles > 1}>
              <div style={{
                display: "flex",
                gap: "3px",
              }}>
                <For each={Array.from({ length: props.downloadProgress!.totalFiles })}>
                  {(_, i) => {
                    const idx = i();
                    const fileIdx = props.downloadProgress!.fileIndex;
                    const isComplete = idx < fileIdx;
                    const isCurrent = idx === fileIdx;
                    return (
                      <div style={{
                        flex: "1",
                        height: "3px",
                        "border-radius": "2px",
                        background: isComplete
                          ? "var(--accent)"
                          : isCurrent
                          ? "var(--accent)"
                          : "var(--bg-tertiary)",
                        opacity: isCurrent ? "0.55" : "1",
                      }} />
                    );
                  }}
                </For>
              </div>
            </Show>
          </div>
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

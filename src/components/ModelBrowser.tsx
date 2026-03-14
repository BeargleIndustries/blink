import { Component, For, createSignal, onMount, onCleanup } from "solid-js";
import type { ModelInfo } from "../lib/types";
import ModelCard from "./ModelCard";
import { importCustomModel } from "../lib/tauri-api";
import { listen } from "@tauri-apps/api/event";

interface ModelBrowserProps {
  models: ModelInfo[];
  downloading: string | null;
  downloadProgress?: { modelId: string; fileRole: string; fileIndex: number; totalFiles: number } | null;
  onDownload: (modelId: string) => void;
  onDelete: (modelId: string) => void;
  onSelect: (modelId: string) => void;
  onClose: () => void;
  vramTotalMb?: number | null;
}

const ModelBrowser: Component<ModelBrowserProps> = (props) => {
  const [importUrl, setImportUrl] = createSignal("");
  const [importing, setImporting] = createSignal(false);
  const [importError, setImportError] = createSignal<string | null>(null);
  const [downloadError, setDownloadError] = createSignal<string | null>(null);

  onMount(async () => {
    const unlisten = await listen<string>("model:download_error", (event) => {
      setDownloadError(event.payload);
      setTimeout(() => setDownloadError(null), 15000);
    });
    onCleanup(unlisten);
  });

  const handleImport = async () => {
    const url = importUrl().trim();
    if (!url) return;
    setImportError(null);
    setImporting(true);
    try {
      await importCustomModel(url);
      setImportUrl("");
    } catch (e: unknown) {
      setImportError(e instanceof Error ? e.message : String(e));
    } finally {
      setImporting(false);
    }
  };

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

        <div style={{ "margin-bottom": "20px" }}>
          <div style={{
            display: "flex",
            gap: "8px",
            "align-items": "center",
          }}>
            <input
              type="text"
              placeholder="Paste HuggingFace URL (e.g. owner/repo:model.gguf)"
              value={importUrl()}
              onInput={(e) => setImportUrl(e.currentTarget.value)}
              onKeyDown={(e) => { if (e.key === "Enter") handleImport(); }}
              disabled={importing()}
              style={{
                flex: "1",
                background: "var(--bg-primary)",
                border: "1px solid var(--border)",
                "border-radius": "6px",
                padding: "8px 12px",
                color: "var(--text-primary)",
                "font-size": "13px",
                outline: "none",
                transition: "border-color 0.15s",
              }}
              onFocus={(e) => { e.currentTarget.style.borderColor = "var(--accent)"; }}
              onBlur={(e) => { e.currentTarget.style.borderColor = "var(--border)"; }}
            />
            <button
              onClick={handleImport}
              disabled={importing() || !importUrl().trim()}
              style={{
                background: "var(--accent)",
                border: "none",
                "border-radius": "6px",
                padding: "8px 16px",
                color: "white",
                "font-size": "13px",
                "font-weight": "600",
                cursor: importing() || !importUrl().trim() ? "not-allowed" : "pointer",
                opacity: importing() || !importUrl().trim() ? "0.6" : "1",
                "white-space": "nowrap",
              }}
            >
              {importing() ? "Importing..." : "Import"}
            </button>
          </div>
          {importError() && (
            <div style={{
              "margin-top": "6px",
              color: "var(--error)",
              "font-size": "12px",
            }}>
              {importError()}
            </div>
          )}
          {importing() && (
            <div style={{
              "margin-top": "6px",
              color: "var(--text-muted)",
              "font-size": "11px",
            }}>
              Downloading from HuggingFace... This may take a while for large models.
            </div>
          )}
        </div>

        {downloadError() && (
          <div style={{
            padding: "8px 12px",
            background: "rgba(239, 68, 68, 0.12)",
            border: "1px solid var(--error)",
            "border-radius": "var(--radius)",
            color: "var(--error)",
            "font-size": "13px",
            "margin-bottom": "16px",
          }}>
            {downloadError()}
          </div>
        )}

        <div style={{
          display: "grid",
          "grid-template-columns": "repeat(auto-fill, minmax(220px, 1fr))",
          gap: "16px",
        }}>
          <For each={props.models}>
            {(model) => (
              <ModelCard
                model={model}
                downloading={props.downloading === model.id}
                downloadProgress={
                  props.downloadProgress?.modelId === model.id
                    ? {
                        fileRole: props.downloadProgress.fileRole,
                        fileIndex: props.downloadProgress.fileIndex,
                        totalFiles: props.downloadProgress.totalFiles,
                      }
                    : null
                }
                onDownload={() => props.onDownload(model.id)}
                onDelete={() => props.onDelete(model.id)}
                onSelect={() => props.onSelect(model.id)}
                vramTotalMb={props.vramTotalMb}
              />
            )}
          </For>
        </div>
      </div>
    </div>
  );
};

export default ModelBrowser;

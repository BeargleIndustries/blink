import { Component, For, Show } from "solid-js";
import type { LoraConfig } from "../lib/types";

interface LoraPickerProps {
  loras: LoraConfig[];
  onLorasChange: (loras: LoraConfig[]) => void;
  visible: boolean;
}

const MAX_LORAS = 5;

const LoraPicker: Component<LoraPickerProps> = (props) => {
  let fileInputRef: HTMLInputElement | undefined;

  const labelStyle = {
    "font-size": "12px",
    color: "var(--text-secondary)",
  } as const;

  const handleFileSelect = (e: Event) => {
    const input = e.currentTarget as HTMLInputElement;
    const files = input.files;
    if (!files || files.length === 0) return;

    const newLoras: LoraConfig[] = [...props.loras];
    for (let i = 0; i < files.length; i++) {
      if (newLoras.length >= MAX_LORAS) break;
      const file = files[i];
      // Use the file path — in Tauri desktop context webkitRelativePath or name
      const path = (file as any).path || file.name;
      // Avoid duplicates
      if (!newLoras.some((l) => l.path === path)) {
        newLoras.push({ path, multiplier: 1.0 });
      }
    }
    props.onLorasChange(newLoras);
    // Reset so the same file can be re-added after removal
    input.value = "";
  };

  const handleRemove = (index: number) => {
    const updated = props.loras.filter((_, i) => i !== index);
    props.onLorasChange(updated);
  };

  const handleMultiplierChange = (index: number, value: number) => {
    const updated = props.loras.map((l, i) =>
      i === index ? { ...l, multiplier: value } : l
    );
    props.onLorasChange(updated);
  };

  const shortName = (path: string) => {
    const parts = path.replace(/\\/g, "/").split("/");
    const filename = parts[parts.length - 1];
    if (filename.length <= 28) return filename;
    return filename.slice(0, 12) + "…" + filename.slice(-12);
  };

  return (
    <Show when={props.visible}>
      <div style={{
        "grid-column": "1 / -1",
        "border-top": "1px solid var(--border)",
        "padding-top": "8px",
        "margin-top": "4px",
      }}>
        <div style={{
          display: "flex",
          "align-items": "center",
          "justify-content": "space-between",
          "margin-bottom": "8px",
        }}>
          <div style={{ display: "flex", "align-items": "center", gap: "8px" }}>
            <span style={{ "font-size": "12px", color: "var(--text-secondary)", "font-weight": "600" }}>
              LoRA
            </span>
            <span style={{ "font-size": "11px", color: "var(--text-muted)", opacity: "0.6" }}>
              {props.loras.length}/{MAX_LORAS}
            </span>
          </div>
          <Show when={props.loras.length < MAX_LORAS}>
            <button
              onClick={() => fileInputRef?.click()}
              style={{
                padding: "3px 10px",
                background: "var(--bg-tertiary)",
                border: "1px solid var(--border)",
                "border-radius": "4px",
                color: "var(--text-secondary)",
                cursor: "pointer",
                "font-size": "11px",
              }}
            >
              + Add LoRA
            </button>
          </Show>
        </div>

        <input
          ref={fileInputRef}
          type="file"
          accept=".safetensors,.gguf"
          multiple
          style={{ display: "none" }}
          onInput={handleFileSelect}
        />

        <Show when={props.loras.length === 0}>
          <div style={{
            "font-size": "11px",
            color: "var(--text-muted)",
            opacity: "0.5",
            padding: "4px 0",
          }}>
            No LoRAs selected. Add .safetensors or .gguf files.
          </div>
        </Show>

        <div style={{ display: "flex", "flex-direction": "column", gap: "6px" }}>
          <For each={props.loras}>
            {(lora, index) => (
              <div style={{
                display: "flex",
                "align-items": "center",
                gap: "8px",
                padding: "6px 8px",
                background: "var(--bg-primary)",
                "border-radius": "4px",
                border: "1px solid var(--border)",
              }}>
                <span
                  title={lora.path}
                  style={{
                    "font-size": "12px",
                    color: "var(--text-primary)",
                    "min-width": "0",
                    flex: "0 0 auto",
                    "white-space": "nowrap",
                    overflow: "hidden",
                    "text-overflow": "ellipsis",
                    "max-width": "140px",
                  }}
                >
                  {shortName(lora.path)}
                </span>
                <input
                  type="range"
                  min="0"
                  max="2"
                  step="0.05"
                  value={lora.multiplier}
                  onInput={(e) =>
                    handleMultiplierChange(index(), parseFloat(e.currentTarget.value))
                  }
                  style={{ flex: "1" }}
                />
                <span style={{ "font-size": "12px", "min-width": "32px", "text-align": "right" }}>
                  {lora.multiplier.toFixed(2)}
                </span>
                <button
                  onClick={() => handleRemove(index())}
                  title="Remove LoRA"
                  style={{
                    padding: "2px 6px",
                    background: "none",
                    border: "1px solid var(--border)",
                    "border-radius": "4px",
                    color: "var(--text-muted)",
                    cursor: "pointer",
                    "font-size": "11px",
                    "flex-shrink": "0",
                  }}
                >
                  ✕
                </button>
              </div>
            )}
          </For>
        </div>
      </div>
    </Show>
  );
};

export default LoraPicker;

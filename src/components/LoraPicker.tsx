import { Component, For, Show } from "solid-js";
import { open } from "@tauri-apps/plugin-dialog";
import type { LoraConfig } from "../lib/types";

interface LoraPickerProps {
  loras: LoraConfig[];
  onLorasChange: (loras: LoraConfig[]) => void;
  visible: boolean;
}

const MAX_LORAS = 5;

const LoraPicker: Component<LoraPickerProps> = (props) => {
  const handleAddLora = async () => {
    const selected = await open({
      multiple: true,
      filters: [{ name: "LoRA Models", extensions: ["safetensors", "gguf"] }],
    });
    if (!selected) return;

    const paths = Array.isArray(selected) ? selected : [selected];
    const newLoras: LoraConfig[] = [...props.loras];
    for (const path of paths) {
      if (newLoras.length >= MAX_LORAS) break;
      if (!newLoras.some((l) => l.path === path)) {
        newLoras.push({ path, multiplier: 1.0 });
      }
    }
    props.onLorasChange(newLoras);
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
      <div>
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
              onClick={handleAddLora}
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
                padding: "6px 8px",
                background: "var(--bg-primary)",
                "border-radius": "4px",
                border: "1px solid var(--border)",
                overflow: "hidden",
              }}>
                <div style={{
                  display: "flex",
                  "align-items": "center",
                  "justify-content": "space-between",
                  "margin-bottom": "4px",
                }}>
                  <span
                    title={lora.path}
                    style={{
                      "font-size": "12px",
                      color: "var(--text-primary)",
                      "white-space": "nowrap",
                      overflow: "hidden",
                      "text-overflow": "ellipsis",
                      "min-width": "0",
                      flex: "1",
                    }}
                  >
                    {shortName(lora.path)}
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
                      "margin-left": "8px",
                    }}
                  >
                    ✕
                  </button>
                </div>
                <div style={{
                  display: "flex",
                  "align-items": "center",
                  gap: "8px",
                }}>
                  <span style={{ "font-size": "10px", color: "var(--text-muted)", "flex-shrink": "0" }}>
                    Strength
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
                    style={{ flex: "1", "min-width": "0" }}
                  />
                  <span style={{
                    "font-size": "12px",
                    "min-width": "32px",
                    "text-align": "right",
                    color: "var(--text-secondary)",
                    "flex-shrink": "0",
                  }}>
                    {lora.multiplier.toFixed(2)}
                  </span>
                </div>
              </div>
            )}
          </For>
        </div>
      </div>
    </Show>
  );
};

export default LoraPicker;

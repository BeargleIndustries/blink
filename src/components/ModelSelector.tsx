import { Component, For } from "solid-js";
import type { ModelInfo } from "../lib/types";

interface ModelSelectorProps {
  models: ModelInfo[];
  activeModelId: string | null;
  onSelectModel: (modelId: string) => void;
}

const ModelSelector: Component<ModelSelectorProps> = (props) => {
  const downloadedModels = () => props.models.filter((m) => m.downloaded);

  return (
    <select
      value={props.activeModelId ?? ""}
      onChange={(e) => props.onSelectModel(e.currentTarget.value)}
      style={{
        padding: "6px 12px",
        background: "var(--bg-tertiary)",
        border: "1px solid var(--border)",
        "border-radius": "var(--radius)",
        color: "var(--text-primary)",
        "font-size": "13px",
        cursor: "pointer",
        outline: "none",
        "min-width": "180px",
      }}
    >
      <option value="" disabled>
        Select a model...
      </option>
      <For each={downloadedModels()}>
        {(model) => <option value={model.id}>{model.name}</option>}
      </For>
    </select>
  );
};

export default ModelSelector;

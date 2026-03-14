import { Component, createSignal } from "solid-js";

interface PromptBarProps {
  onGenerate: (prompt: string, negativePrompt: string) => void;
  onCancel: () => void;
  generating: boolean;
  modelLoading: boolean;
  modelReady: boolean;
  mode: "txt2img" | "img2img";
}

const PromptBar: Component<PromptBarProps> = (props) => {
  const [prompt, setPrompt] = createSignal("");
  const [negativePrompt, setNegativePrompt] = createSignal("");

  const canGenerate = () => props.modelReady && !props.generating && !props.modelLoading && prompt().trim().length > 0;

  const handleGenerate = () => {
    if (canGenerate()) {
      props.onGenerate(prompt(), negativePrompt());
    }
  };

  const handleKeyDown = (e: KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleGenerate();
    }
  };

  return (
    <div style={{
      width: "100%",
      "max-width": "700px",
      display: "flex",
      "flex-direction": "column",
      gap: "8px",
    }}>
      <textarea
        placeholder="Enter your prompt..."
        rows={3}
        value={prompt()}
        onInput={(e) => setPrompt(e.currentTarget.value)}
        onKeyDown={handleKeyDown}
        disabled={props.generating}
        style={{
          width: "100%",
          padding: "12px",
          background: "var(--bg-secondary)",
          border: "1px solid var(--border)",
          "border-radius": "var(--radius)",
          color: "var(--text-primary)",
          resize: "vertical",
          "font-size": "14px",
          "font-family": "inherit",
          outline: "none",
          "box-sizing": "border-box",
        }}
      />
      <input
        type="text"
        placeholder="Negative prompt (optional)"
        value={negativePrompt()}
        onInput={(e) => setNegativePrompt(e.currentTarget.value)}
        disabled={props.generating}
        style={{
          width: "100%",
          padding: "8px 12px",
          background: "var(--bg-secondary)",
          border: "1px solid var(--border)",
          "border-radius": "var(--radius)",
          color: "var(--text-primary)",
          "font-size": "13px",
          outline: "none",
          "box-sizing": "border-box",
        }}
      />
      <button
        onClick={props.generating ? props.onCancel : handleGenerate}
        disabled={!props.generating && !canGenerate()}
        style={{
          padding: "12px 24px",
          background: props.generating ? "var(--error)" : props.modelLoading ? "var(--bg-tertiary)" : "var(--accent)",
          border: "none",
          "border-radius": "var(--radius)",
          color: "white",
          "font-size": "16px",
          "font-weight": "bold",
          cursor: !props.generating && !canGenerate() ? "not-allowed" : "pointer",
          opacity: !props.generating && !canGenerate() ? "0.5" : "1",
          transition: "var(--transition)",
        }}
      >
        {props.modelLoading
          ? "Loading model..."
          : !props.modelReady
          ? "Select a model"
          : props.generating
          ? "Cancel"
          : props.mode === "img2img"
          ? "Transform"
          : "Generate"}
      </button>
    </div>
  );
};

export default PromptBar;

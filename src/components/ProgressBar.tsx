import { Component, Show } from "solid-js";

interface ProgressBarProps {
  step: number;
  totalSteps: number;
  visible: boolean;
  elapsed?: number;
}

const ProgressBar: Component<ProgressBarProps> = (props) => {
  const percentage = () =>
    props.totalSteps > 0 ? (props.step / props.totalSteps) * 100 : 0;

  const formatElapsed = () => {
    const s = props.elapsed ?? 0;
    if (s < 60) return `${s.toFixed(1)}s`;
    const m = Math.floor(s / 60);
    const rem = (s % 60).toFixed(0).padStart(2, "0");
    return `${m}:${rem}`;
  };

  return (
    <Show when={props.visible}>
      <div style={{
        width: "512px",
        "max-width": "100%",
      }}>
        <div style={{
          height: "4px",
          background: "var(--bg-tertiary)",
          "border-radius": "2px",
          overflow: "hidden",
        }}>
          <div style={{
            height: "100%",
            width: `${percentage()}%`,
            background: "var(--accent)",
            transition: "width 0.25s ease",
            "border-radius": "2px",
            "box-shadow": "0 0 8px var(--accent)",
          }} />
        </div>
        <div style={{
          display: "flex",
          "justify-content": "space-between",
          "align-items": "center",
          "margin-top": "5px",
          "font-size": "11px",
          color: "var(--text-secondary)",
        }}>
          <span>Step {props.step} / {props.totalSteps}</span>
          <span>{Math.round(percentage())}%</span>
          <Show when={(props.elapsed ?? 0) > 0}>
            <span>{formatElapsed()}</span>
          </Show>
        </div>
      </div>
    </Show>
  );
};

export default ProgressBar;

import { Component, Show, createMemo } from "solid-js";

export type ProgressPhase = "loading" | "sampling" | "decoding";

interface ProgressBarProps {
  step: number;
  totalSteps: number;
  visible: boolean;
  elapsed?: number;  // per-step time from sd.cpp
  phase?: ProgressPhase;
  modelLabel?: string;
  modelBytes?: number;
}

/** "6.3 GB", or an empty string when the size is unknown. */
const formatGiB = (bytes?: number) => {
  if (!bytes || bytes <= 0) return "";
  return `${(bytes / 1024 ** 3).toFixed(1)} GB`;
};

const ProgressBar: Component<ProgressBarProps> = (props) => {
  const phase = () => props.phase ?? "sampling";

  const percentage = () =>
    props.totalSteps > 0 ? (props.step / props.totalSteps) * 100 : 0;

  // Estimate total and remaining time based on average step time.
  // Only meaningful while sampling: during a load the counter is a tensor
  // count, and a tensor-load rate is not a time estimate.
  const timeInfo = createMemo(() => {
    if (phase() !== "sampling") return null;
    const stepTime = props.elapsed ?? 0;
    const step = props.step;
    const total = props.totalSteps;
    if (step <= 0 || total <= 0 || stepTime <= 0) return null;

    // Use current step time as estimate per step (smooths out over time)
    const remaining = stepTime * (total - step);
    const totalEstimate = stepTime * total;
    return { stepTime, remaining, totalEstimate };
  });

  const formatTime = (s: number) => {
    if (s < 60) return `${s.toFixed(1)}s`;
    const m = Math.floor(s / 60);
    const rem = (s % 60).toFixed(0).padStart(2, "0");
    return `${m}:${rem}`;
  };

  // The left-hand label. During a load this must never expose the underlying
  // counter — it counts tensors, which is an engine internal.
  const label = () => {
    if (phase() === "decoding") return "Decoding image…";
    if (phase() === "loading") {
      const name = props.modelLabel ?? "the model";
      const size = formatGiB(props.modelBytes);
      const sized = size ? `${name} (${size})` : name;
      return `Loading ${sized} — first run reads the model from disk and can take a minute`;
    }
    return `Step ${props.step} / ${props.totalSteps}`;
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
          <span>{label()}</span>
          <Show when={phase() !== "decoding"}>
            <span>{Math.round(percentage())}%</span>
          </Show>
          <Show when={timeInfo()}>
            {(info) => (
              <span>
                ~{formatTime(info().remaining)} left ({formatTime(info().stepTime)}/step)
              </span>
            )}
          </Show>
        </div>
      </div>
    </Show>
  );
};

export default ProgressBar;

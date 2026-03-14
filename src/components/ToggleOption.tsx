import { Component } from "solid-js";

const ToggleOption: Component<{
  label: string;
  tooltip: string;
  checked: boolean;
  onChange: (v: boolean) => void;
}> = (props) => (
  <label
    title={props.tooltip}
    style={{
      display: "flex",
      "align-items": "center",
      gap: "6px",
      cursor: "pointer",
      "font-size": "12px",
      color: "var(--text-primary)",
    }}
  >
    <input
      type="checkbox"
      checked={props.checked}
      onChange={(e) => props.onChange(e.currentTarget.checked)}
      style={{ "accent-color": "var(--accent)" }}
    />
    {props.label}
  </label>
);

export default ToggleOption;

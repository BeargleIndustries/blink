import { Component } from "solid-js";

interface LicenseInfoProps {
  name: string;
  url: string;
  commercial: boolean;
}

const LicenseInfo: Component<LicenseInfoProps> = (props) => {
  const handleClick = (e: MouseEvent) => {
    e.preventDefault();
    import("@tauri-apps/plugin-shell").then(({ open }) => open(props.url));
  };

  return (
    <div style={{
      display: "flex",
      "align-items": "center",
      gap: "6px",
      "font-size": "11px",
    }}>
      <span style={{
        padding: "2px 6px",
        "border-radius": "var(--radius-pill)",
        background: props.commercial
          ? "rgba(34, 197, 94, 0.1)"
          : "rgba(239, 68, 68, 0.1)",
        border: props.commercial
          ? "1px solid rgba(34, 197, 94, 0.2)"
          : "1px solid rgba(239, 68, 68, 0.2)",
        color: props.commercial ? "var(--success)" : "var(--error)",
        "font-weight": "500",
      }}>
        {props.commercial ? "Commercial OK" : "Non-Commercial"}
      </span>
      <a
        href={props.url}
        target="_blank"
        rel="noopener"
        onClick={handleClick}
        style={{
          color: "var(--text-secondary)",
          "text-decoration": "underline",
        }}
      >
        {props.name}
      </a>
    </div>
  );
};

export default LicenseInfo;

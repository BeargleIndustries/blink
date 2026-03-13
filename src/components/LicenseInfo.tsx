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
        "border-radius": "4px",
        background: props.commercial
          ? "rgba(76, 175, 80, 0.2)"
          : "rgba(244, 67, 54, 0.2)",
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

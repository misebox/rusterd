/// The few values every page shares. Everything else is local to its page.
export const theme = {
  ink: "#222",
  quiet: "#666",
  faint: "#888",
  rule: "#ccc",
  paper: "#fff",
  panel: "#fafafa",
  accent: "#0b6",
  sans: "system-ui, -apple-system, Segoe UI, sans-serif",
  mono: "ui-monospace, SFMono-Regular, Menlo, monospace",
  width: "1100px",
};

export const page = {
  "font-family": theme.sans,
  color: theme.ink,
  "line-height": "1.6",
} as const;

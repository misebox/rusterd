import { JSX, Show } from "solid-js";
import { theme } from "./theme";

/// A row of tabs. What is under them is the caller's business — this only says
/// which one is being asked for.
export default function Tabs(props: {
  names: readonly string[];
  shown: string;
  choose: (name: string) => void;
  /// What to write on a tab, when it is not the name itself.
  label?: (name: string) => string;
  /// Anything to put at the far end of the row.
  trailing?: JSX.Element;
}) {
  return (
    <div style={styles.bar}>
      {props.names.map((name) => (
        <button
          style={{ ...styles.tab, ...(props.shown === name ? styles.chosen : {}) }}
          onClick={() => props.choose(name)}
        >
          {props.label ? props.label(name) : name}
        </button>
      ))}
      <Show when={props.trailing}>
        <span style={styles.trailing}>{props.trailing}</span>
      </Show>
    </div>
  );
}

const styles = {
  bar: {
    display: "flex",
    "align-items": "center",
    gap: "4px",
    "border-bottom": `1px solid ${theme.rule}`,
    "margin-bottom": "12px",
  },
  tab: {
    "font-family": theme.sans,
    "font-size": "14px",
    padding: "7px 14px",
    border: "1px solid transparent",
    "border-bottom": "none",
    "border-radius": "4px 4px 0 0",
    background: "transparent",
    color: theme.quiet,
    cursor: "pointer",
  },
  chosen: {
    border: `1px solid ${theme.rule}`,
    "border-bottom": `1px solid ${theme.paper}`,
    "margin-bottom": "-1px",
    background: theme.paper,
    color: theme.ink,
    "font-weight": "bold",
  },
  trailing: {
    "margin-left": "auto",
    "font-size": "13px",
  },
} as const;

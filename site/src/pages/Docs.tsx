import { createSignal, onMount, Show } from "solid-js";
import init from "../../../pkg/rusterd.js";
import overview from "../../../README.md?raw";
import language from "../../../docs/DSL-spec.md?raw";
import erdGrammar from "../../../docs/erd.gbnf?url";
import sqlGrammar from "../../../docs/sql.gbnf?url";
import Header from "../Header";
import Markdown from "../Markdown";
import { intro, sections } from "../document";
import "../prose.css";
import { theme } from "../theme";

/// The three documents, told apart by the file the page was opened as. Reading
/// the README twice, for different parts of it, is what keeps the front page
/// short without saying anything twice.
const DOCUMENTS = {
  "index.html": {
    lead: intro(overview),
    body: sections(overview, ["Features", "Example"]),
  },
  "start.html": {
    lead: "",
    body: sections(overview, [
      "Install",
      "CLI Usage",
      "Browser Usage (WASM)",
      "Rust Library Usage",
    ]),
  },
  "language.html": { lead: "", body: language },
};

const here = (["start.html", "language.html"] as const).find((name) =>
  location.pathname.endsWith(name),
);
const HERE = here ?? "index.html";
const DOCUMENT = DOCUMENTS[HERE];

export default function Docs() {
  const [ready, setReady] = createSignal(false);

  onMount(async () => {
    await init();
    setReady(true);
  });

  return (
    <div style={styles.container}>
      <Header here={HERE} />

      <Show when={HERE === "index.html"}>
        <div style={styles.lead}>
          <p style={styles.tagline}>{DOCUMENT.lead}</p>
          <div style={styles.actions}>
            <a style={{ ...styles.button, ...styles.first }} href="start.html">
              Get started
            </a>
            <a style={styles.button} href="demo.html">
              Try it in the browser
            </a>
          </div>
        </div>
      </Show>

      <main>
        <Show when={HERE === "language.html"}>
          <div style={styles.grammars}>
            <span style={styles.grammarsLabel}>Grammars for constrained decoding</span>
            <a style={styles.file} href={erdGrammar} download="erd.gbnf">
              erd.gbnf
            </a>
            <a style={styles.file} href={sqlGrammar} download="sql.gbnf">
              sql.gbnf
            </a>
          </div>
        </Show>
        <Markdown source={DOCUMENT.body} ready={ready()} />
      </main>

      <footer style={styles.footer}>
        Every diagram on this page was compiled in your browser, by the same
        code the command line runs.
      </footer>
    </div>
  );
}

const styles = {
  container: {
    "font-family": theme.sans,
    color: theme.ink,
    "max-width": theme.width,
    margin: "0 auto",
    padding: "0 20px 80px",
  },
  lead: {
    "text-align": "center",
    margin: "12px 0 44px",
  },
  tagline: {
    "font-size": "19px",
    "line-height": "1.6",
    color: theme.quiet,
    "max-width": "620px",
    margin: "0 auto 24px",
  },
  actions: {
    display: "flex",
    "justify-content": "center",
    "flex-wrap": "wrap",
    gap: "12px",
  },
  button: {
    "font-size": "15px",
    padding: "9px 20px",
    border: `1px solid ${theme.rule}`,
    "border-radius": "6px",
    background: theme.paper,
    color: theme.ink,
    "text-decoration": "none",
  },
  first: {
    background: theme.ink,
    "border-color": theme.ink,
    color: theme.paper,
  },
  grammars: {
    display: "flex",
    "align-items": "center",
    gap: "12px",
    "flex-wrap": "wrap",
    padding: "12px 16px",
    "margin-bottom": "28px",
    border: `1px solid ${theme.rule}`,
    "border-radius": "6px",
    background: theme.panel,
    "font-size": "14px",
  },
  grammarsLabel: {
    color: theme.quiet,
    "margin-right": "auto",
  },
  file: {
    "font-family": theme.mono,
    "font-size": "13px",
    padding: "4px 10px",
    border: `1px solid ${theme.rule}`,
    "border-radius": "4px",
    background: theme.paper,
    color: theme.ink,
    "text-decoration": "none",
  },
  footer: {
    "margin-top": "60px",
    "padding-top": "20px",
    "border-top": `1px solid ${theme.rule}`,
    "font-size": "13px",
    color: theme.faint,
  },
} as const;

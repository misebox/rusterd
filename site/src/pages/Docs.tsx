import { createSignal, onMount, Show } from "solid-js";
import init from "../../../pkg/rusterd.js";
import overview from "../../../README.md?raw";
import language from "../../../docs/DSL-spec.md?raw";
import erdGrammar from "../../../docs/erd.gbnf?url";
import sqlGrammar from "../../../docs/sql.gbnf?url";
import Header from "../Header";
import Markdown from "../Markdown";
import "../prose.css";
import { theme } from "../theme";

/// Which document this page shows, chosen by the file it was opened as. Two
/// pages, one component: they differ only in what they are reading.
const DOCUMENT = location.pathname.endsWith("language.html")
  ? { here: "language.html", source: language }
  : { here: "index.html", source: readme() };

/// The README as a page rather than as a repository front door: the line
/// pointing at this site is only useful from the other side of the link.
function readme(): string {
  return overview
    .split("\n")
    .filter((line) => !line.startsWith("Live demo:"))
    .join("\n");
}

export default function Docs() {
  const [ready, setReady] = createSignal(false);

  onMount(async () => {
    await init();
    setReady(true);
  });

  return (
    <div style={styles.container}>
      <Header here={DOCUMENT.here} />
      <main>
        <Show when={DOCUMENT.here === "language.html"}>
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
        <Markdown source={DOCUMENT.source} ready={ready()} />
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

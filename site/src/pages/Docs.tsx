import { createSignal, onMount, Show } from "solid-js";
import init from "../../../pkg/rusterd.js";
import overview from "../../../README.md?raw";
import reference from "../../../docs/DSL-spec.md?raw";
import erdGrammar from "../../../docs/erd.gbnf?url";
import sqlGrammar from "../../../docs/sql.gbnf?url";
import Header from "../Header";
import Markdown from "../Markdown";
import { chunks, intro, sections, withoutHeading } from "../document";
import { inLanguage, language, say } from "../i18n";
import "../prose.css";
import { theme } from "../theme";

/// The documents that have been translated. A page with nothing here is shown
/// in English, which is where they are all written.
const TRANSLATED = translations(
  import.meta.glob("../../../docs/ja/*.md", { query: "?raw", eager: true }),
);

function translations(modules: Record<string, unknown>): Record<string, string> {
  const found: Record<string, string> = {};
  for (const [path, module] of Object.entries(modules)) {
    const name = path.split("/").pop()?.replace(/\.md$/, "");
    if (name) {
      found[name] = (module as { default: string }).default;
    }
  }
  return found;
}

/// The three documents, told apart by the file the page was opened as. Reading
/// the README twice, for different parts of it, is what keeps the front page
/// short without saying anything twice.
const DOCUMENTS = {
  "index.html": {
    name: "overview",
    lead: intro(overview),
    body: sections(overview, ["Features", "Example"]),
  },
  "start.html": {
    name: "start",
    lead: "",
    body: sections(overview, [
      "Install",
      "CLI Usage",
      "Browser Usage (WASM)",
      "Rust Library Usage",
    ]),
  },
  "language.html": { name: "language", lead: "", body: reference },
};

const here = (["start.html", "language.html"] as const).find((name) =>
  location.pathname.endsWith(name),
);
const HERE = here ?? "index.html";
const DOCUMENT = DOCUMENTS[HERE];
const LANGUAGE = language();
const TRANSLATION = LANGUAGE === "en" ? undefined : TRANSLATED[DOCUMENT.name];
const TAGLINE =
  LANGUAGE === "ja"
    ? "テキストファイルから ER 図を描くコンパイラ。Rust で書かれ、ブラウザ向けに WASM にコンパイルされます。"
    : DOCUMENT.lead;

/// The front page reads its two sections in the order they are written —
/// features, then the example — and shows them the other way round. What the
/// compiler does is quicker to see than to read about, so the diagram comes
/// first and the list of features stands behind it as the detail.
const [FEATURES, EXAMPLE] = chunks(TRANSLATION ?? DOCUMENT.body);

export default function Docs() {
  const [ready, setReady] = createSignal(false);

  onMount(async () => {
    await init();
    setReady(true);
  });

  return (
    <div style={styles.container}>
      <Header here={HERE} language={LANGUAGE} />

      <Show when={HERE === "index.html"}>
        <p style={styles.tagline}>{TAGLINE}</p>
      </Show>

      <main>
        <Show when={HERE === "language.html"}>
          <div style={styles.grammars}>
            <span style={styles.grammarsLabel}>{say("grammars", LANGUAGE)}</span>
            <a style={styles.file} href={erdGrammar} download="erd.gbnf">
              erd.gbnf
            </a>
            <a style={styles.file} href={sqlGrammar} download="sql.gbnf">
              sql.gbnf
            </a>
          </div>
        </Show>
        <Show when={LANGUAGE !== "en" && !TRANSLATION}>
          <p style={styles.notice}>{say("untranslated", LANGUAGE)}</p>
        </Show>
        <Show
          when={HERE === "index.html" && EXAMPLE}
          fallback={
            <Markdown
              source={TRANSLATION ?? DOCUMENT.body}
              layout={HERE === "language.html" ? "tabs" : "beside"}
              ready={ready()}
              language={LANGUAGE}
            />
          }
        >
          <Markdown
            source={withoutHeading(EXAMPLE)}
            layout="beside"
            ready={ready()}
            language={LANGUAGE}
          />
          <Markdown source={FEATURES} layout="beside" ready={ready()} language={LANGUAGE} />
          <div style={styles.actions}>
            <a
              style={{ ...styles.button, ...styles.first }}
              href={inLanguage("start.html", LANGUAGE)}
            >
              {say("getStarted", LANGUAGE)}
            </a>
            <a style={styles.button} href={inLanguage("demo.html", LANGUAGE)}>
              {say("tryIt", LANGUAGE)}
            </a>
          </div>
        </Show>
      </main>

      <footer style={styles.footer}>{say("compiledHere", LANGUAGE)}</footer>
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
  tagline: {
    "font-size": "19px",
    "line-height": "1.6",
    "text-align": "center",
    color: theme.quiet,
    "max-width": "620px",
    margin: "12px auto 36px",
  },
  actions: {
    display: "flex",
    "justify-content": "center",
    "flex-wrap": "wrap",
    gap: "12px",
    "margin-top": "40px",
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
  notice: {
    "font-size": "14px",
    color: theme.quiet,
    padding: "10px 14px",
    "margin-bottom": "24px",
    border: `1px solid ${theme.rule}`,
    "border-radius": "6px",
    background: theme.panel,
  },
  footer: {
    "margin-top": "60px",
    "padding-top": "20px",
    "border-top": `1px solid ${theme.rule}`,
    "font-size": "13px",
    color: theme.faint,
  },
} as const;

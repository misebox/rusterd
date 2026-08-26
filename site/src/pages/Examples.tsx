import { createMemo, createSignal, onMount, Show } from "solid-js";
import init, { erdToSvg } from "../../../pkg/rusterd.js";
import Header from "../Header";
import "../diagram.css";
import { theme } from "../theme";

/// The example files the repository ships, read at build time so the page can
/// never fall behind them.
const SOURCES = load(import.meta.glob("../../../examples/*.erd", { query: "?raw", eager: true }));
const DUMPS = load(import.meta.glob("../../../examples/*.sql", { query: "?raw", eager: true }));

function load(modules: Record<string, unknown>): Record<string, string> {
  const found: Record<string, string> = {};
  for (const [path, module] of Object.entries(modules)) {
    const name = path.split("/").pop()?.replace(/\.[a-z]+$/, "");
    if (name) {
      found[name] = (module as { default: string }).default;
    }
  }
  return found;
}

const NAMES = Object.keys(SOURCES).sort();

/// What each example is there to show. Anything not named here is listed by its
/// file name alone.
const ABOUT: Record<string, string> = {
  "01_many_columns": "An entity with more columns than fit comfortably",
  "02_wide_horizontal": "Entities wide enough to push the diagram sideways",
  "03_long_names": "Names long enough to set the width on their own",
  "04_deep_hierarchy": "A chain of entities, each below the last",
  "05_dense_relations": "Enough relationships to crowd every channel",
  "06_unicode_cjk": "Japanese names, measured by display width",
  "07_all_cardinalities": "One of each: 1, 0..1, *, 1..*",
  "08_mixed_sizes": "Entities of very different heights side by side",
  "09_orphan_entities": "Entities with nothing attached to them",
  "10_ecommerce_full": "A shop: the shape most schemas end up",
  "11_near": "@hint.near, keeping related entities together",
  "21_idp": "An identity provider, converted from its SQL dump",
  sample: "The one in the README",
};

const TABS = ["Diagram", "ERD", "SVG", "SQL"] as const;
type Tab = (typeof TABS)[number];

export default function Examples() {
  const opened = decodeURIComponent(location.hash.slice(1));
  const [chosen, setChosen] = createSignal(NAMES.includes(opened) ? opened : NAMES[0]);
  const [tab, setTab] = createSignal<Tab>("Diagram");
  const [ready, setReady] = createSignal(false);

  onMount(async () => {
    await init();
    setReady(true);
  });

  const erd = createMemo(() => SOURCES[chosen()] ?? "");
  const sql = createMemo(() => DUMPS[chosen()]);
  const svg = createMemo(() => {
    if (!ready()) {
      return "";
    }
    try {
      return erdToSvg(erd(), null, null, null, null, null);
    } catch (e) {
      return `<!-- ${String(e)} -->`;
    }
  });

  const show = (name: string) => {
    setChosen(name);
    setTab("Diagram");
    history.replaceState(null, "", `#${name}`);
  };

  const shown = createMemo(() => TABS.filter((name) => name !== "SQL" || sql()));

  return (
    <div style={styles.container}>
      <Header here="examples.html" />

      <p style={styles.blurb}>
        Every file in <code>examples/</code>, compiled here in the browser. The
        diagrams are what <code>rusterd render</code> writes for the same input.
      </p>

      <div style={styles.layout}>
        <nav style={styles.list}>
          {NAMES.map((name) => (
            <button
              style={{
                ...styles.entry,
                ...(name === chosen() ? styles.entryChosen : {}),
              }}
              onClick={() => show(name)}
            >
              <span style={styles.entryName}>{name}</span>
              <Show when={ABOUT[name]}>
                <span style={styles.entryAbout}>{ABOUT[name]}</span>
              </Show>
            </button>
          ))}
        </nav>

        <section style={styles.viewer}>
          <div style={styles.tabs}>
            {shown().map((name) => (
              <button
                style={{ ...styles.tab, ...(tab() === name ? styles.tabActive : {}) }}
                onClick={() => setTab(name)}
              >
                {name}
              </button>
            ))}
            <a
              style={styles.download}
              href={`https://github.com/misebox/rusterd/blob/main/examples/${chosen()}.erd`}
              target="_blank"
              rel="noopener noreferrer"
            >
              Source
            </a>
          </div>

          <Show when={tab() === "Diagram"}>
            <div class="diagram" style={styles.drawing} innerHTML={svg()} />
          </Show>
          <Show when={tab() === "ERD"}>
            <pre style={styles.code}>{erd()}</pre>
          </Show>
          <Show when={tab() === "SVG"}>
            <pre style={styles.code}>{svg()}</pre>
          </Show>
          <Show when={tab() === "SQL"}>
            <pre style={styles.code}>{sql()}</pre>
          </Show>
        </section>
      </div>
    </div>
  );
}

const styles = {
  container: {
    "font-family": theme.sans,
    color: theme.ink,
    "max-width": "1400px",
    margin: "0 auto",
    padding: "0 20px 60px",
  },
  blurb: {
    "font-size": "15px",
    color: theme.quiet,
    margin: "0 0 20px",
  },
  layout: {
    display: "flex",
    gap: "24px",
    "align-items": "flex-start",
    "flex-wrap": "wrap",
  },
  list: {
    display: "flex",
    "flex-direction": "column",
    gap: "2px",
    "min-width": "260px",
    flex: "0 1 300px",
  },
  entry: {
    display: "flex",
    "flex-direction": "column",
    gap: "2px",
    "text-align": "left",
    padding: "8px 10px",
    border: "1px solid transparent",
    "border-radius": "6px",
    background: "transparent",
    cursor: "pointer",
    "font-family": theme.sans,
  },
  entryChosen: {
    border: `1px solid ${theme.rule}`,
    background: theme.panel,
  },
  entryName: {
    "font-family": theme.mono,
    "font-size": "13px",
    color: theme.ink,
  },
  entryAbout: {
    "font-size": "12px",
    color: theme.faint,
    "line-height": "1.4",
  },
  viewer: {
    flex: "1 1 640px",
    "min-width": "0",
  },
  tabs: {
    display: "flex",
    "align-items": "center",
    gap: "4px",
    "border-bottom": `1px solid ${theme.rule}`,
    "margin-bottom": "12px",
  },
  tab: {
    "font-family": theme.sans,
    "font-size": "14px",
    padding: "8px 14px",
    border: "1px solid transparent",
    "border-bottom": "none",
    "border-radius": "4px 4px 0 0",
    background: "transparent",
    color: theme.quiet,
    cursor: "pointer",
  },
  tabActive: {
    border: `1px solid ${theme.rule}`,
    "border-bottom": `1px solid ${theme.paper}`,
    "margin-bottom": "-1px",
    background: theme.paper,
    color: theme.ink,
    "font-weight": "bold",
  },
  download: {
    "margin-left": "auto",
    "font-size": "13px",
    color: theme.quiet,
    "text-decoration": "none",
  },
  drawing: {
    border: `1px solid ${theme.rule}`,
    "border-radius": "6px",
    padding: "16px",
    background: theme.paper,
    overflow: "auto",
    "max-height": "78vh",
  },
  code: {
    "font-family": theme.mono,
    "font-size": "13px",
    "line-height": "1.55",
    border: `1px solid ${theme.rule}`,
    "border-radius": "6px",
    padding: "16px",
    background: theme.panel,
    overflow: "auto",
    "max-height": "78vh",
    margin: "0",
  },
} as const;

import { createMemo, createSignal, onCleanup, onMount, Show } from "solid-js";
import init, { erdToSvg } from "../../../pkg/rusterd.js";
import Header from "../Header";
import Tabs from "../Tabs";
import { language, say } from "../i18n";
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
const ABOUT: Record<string, { en: string; ja: string }> = {
  "01_many_columns": {
    en: "An entity with more columns than fit comfortably",
    ja: "列が多すぎるエンティティ",
  },
  "02_wide_horizontal": {
    en: "Entities wide enough to push the diagram sideways",
    ja: "横に広がるエンティティ",
  },
  "03_long_names": {
    en: "Names long enough to set the width on their own",
    ja: "名前だけで幅が決まる場合",
  },
  "04_deep_hierarchy": {
    en: "A chain of entities, each below the last",
    ja: "下へ連なるエンティティの鎖",
  },
  "05_dense_relations": {
    en: "Enough relationships to crowd every channel",
    ja: "チャネルが混み合うほどの関係",
  },
  "06_unicode_cjk": {
    en: "Japanese names, measured by display width",
    ja: "日本語の名前を表示幅で測る",
  },
  "07_all_cardinalities": {
    en: "One of each: 1, 0..1, *, 1..*",
    ja: "多重度 4 種類の見本",
  },
  "08_mixed_sizes": {
    en: "Entities of very different heights side by side",
    ja: "高さの違うエンティティが並ぶ",
  },
  "09_orphan_entities": {
    en: "Entities with nothing attached to them",
    ja: "何にも繋がっていないエンティティ",
  },
  "10_ecommerce_full": {
    en: "A shop: the shape most schemas end up",
    ja: "ショップ。よくあるスキーマの形",
  },
  "11_near": {
    en: "@hint.near, keeping related entities together",
    ja: "@hint.near で近くに置く",
  },
  "21_idp": {
    en: "An identity provider, converted from its SQL dump",
    ja: "認証基盤。SQL ダンプから変換したもの",
  },
  sample: { en: "The one in the README", ja: "README に載せているもの" },
};

/// The example the address asks for, if it is one the repository ships.
function asked(): string | undefined {
  const name = decodeURIComponent(location.hash.slice(1));
  return NAMES.includes(name) ? name : undefined;
}

const LANGUAGE = language();

/// Left to right is the order the compiler works in: a SQL dump converts to
/// ERD, which compiles to SVG, which the browser draws. The one worth seeing
/// first is the last of them, so that is where a page opens.
const TABS = ["SQL", "ERD", "SVG", "Diagram"] as const;
const FIRST: Tab = "Diagram";
type Tab = (typeof TABS)[number];

export default function Examples() {
  const [chosen, setChosen] = createSignal(asked() ?? NAMES[0]);
  const [tab, setTab] = createSignal<Tab>(FIRST);
  const [ready, setReady] = createSignal(false);

  onMount(async () => {
    await init();
    setReady(true);
  });

  // Changing only the fragment does not reload the page, so the example named
  // in it has to be followed by hand.
  onMount(() => {
    const follow = () => {
      const name = asked();
      if (name) {
        setChosen(name);
        setTab(FIRST);
      }
    };
    window.addEventListener("hashchange", follow);
    onCleanup(() => window.removeEventListener("hashchange", follow));
  });

  const erd = createMemo(() => SOURCES[chosen()] ?? "");
  const sql = createMemo(() => DUMPS[chosen()]);
  const svg = createMemo(() => {
    if (!ready()) {
      return "";
    }
    try {
      return erdToSvg(erd());
    } catch (e) {
      return `<!-- ${String(e)} -->`;
    }
  });

  const show = (name: string) => {
    setChosen(name);
    setTab(FIRST);
    history.replaceState(null, "", `#${name}`);
  };

  const shown = createMemo(() => TABS.filter((name) => name !== "SQL" || sql()));

  return (
    <div style={styles.container}>
      <Header here="examples.html" language={LANGUAGE} />

      <p style={styles.blurb}>{say("everyExample", LANGUAGE)}</p>

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
                <span style={styles.entryAbout}>{ABOUT[name][LANGUAGE]}</span>
              </Show>
            </button>
          ))}
        </nav>

        <section style={styles.viewer}>
          <Tabs
            names={shown()}
            shown={tab()}
            choose={(name) => setTab(name as Tab)}
            label={(name) => (name === "Diagram" ? say("diagram", LANGUAGE) : name)}
            trailing={
              <a
                style={styles.download}
                href={`https://github.com/misebox/rusterd/blob/main/examples/${chosen()}.erd`}
                target="_blank"
                rel="noopener noreferrer"
              >
                {say("source", LANGUAGE)}
              </a>
            }
          />

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
  download: {
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

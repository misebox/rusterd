import { createMemo, createSignal, onCleanup, onMount, Show } from "solid-js";
import init, { erdToSvg, sqlToErd, type Dialect } from "../../../pkg/rusterd.js";
import Controls, { Drawing, Named, PLAIN, flagsFor, oneOf, styles as control } from "../Drawing";
import Header from "../Header";
import Tabs from "../Tabs";
import { language, say } from "../i18n";
import { inRepo } from "../project";
import "../diagram.css";
import "../workbench.css";
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
  "12_many_leaves": {
    en: "Nine leaves on one parent — change Shape to fold them",
    ja: "1 つの親に葉が 9 つ。形を変えると折り返す",
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

const DIALECTS: { value: Dialect; label: string }[] = [
  { value: "auto", label: "Auto-detect" },
  { value: "postgres", label: "PostgreSQL" },
  { value: "mysql", label: "MySQL" },
  { value: "generic", label: "Generic" },
];

export default function Examples() {
  const [chosen, setChosen] = createSignal(asked() ?? NAMES[0]);
  const [tab, setTab] = createSignal<Tab>(FIRST);
  const [actual, setActual] = createSignal(false);
  const [drawing, setDrawing] = createSignal<Drawing>(PLAIN);
  const [dialect, setDialect] = createSignal<Dialect>("auto");
  const [ready, setReady] = createSignal(false);

  // What the page is showing, which starts as what the repository ships and
  // becomes whatever the reader types over it.
  const [erd, setErd] = createSignal("");
  const [svg, setSvg] = createSignal("");
  const [sql, setSql] = createSignal("");
  const [error, setError] = createSignal("");

  const shipped = () => SOURCES[chosen()] ?? "";
  const edited = () => erd() !== shipped();

  /// Compile, showing whatever the compiler complains about rather than
  /// leaving the diagram silently stale.
  const compile = (source: string) => {
    if (!ready()) {
      return;
    }
    try {
      setSvg(erdToSvg(source, drawing()));
      setError("");
    } catch (e) {
      setError(String(e));
    }
  };

  /// Load an example over whatever is on the page.
  const open = (name: string) => {
    setSql(DUMPS[name] ?? "");
    setErd(SOURCES[name] ?? "");
    setError("");
    compile(erd());
  };

  const write = (source: string) => {
    setErd(source);
    compile(source);
  };

  const convert = () => {
    try {
      const converted = sqlToErd(sql(), dialect());
      if (!converted.trim()) {
        throw new Error("No tables found. Check the SQL, or choose the dialect explicitly.");
      }
      write(converted);
      setTab("ERD");
    } catch (e) {
      setError(String(e));
    }
  };

  onMount(async () => {
    await init();
    setReady(true);
    open(chosen());
  });

  // Changing only the fragment does not reload the page, so the example named
  // in it has to be followed by hand.
  onMount(() => {
    const follow = () => {
      const name = asked();
      if (name) {
        setChosen(name);
        setTab(FIRST);
        open(name);
      }
    };
    window.addEventListener("hashchange", follow);
    onCleanup(() => window.removeEventListener("hashchange", follow));
  });

  // A diagram of its own, for the browser to zoom, save or print — which it
  // does better than anything this page could offer.
  const alone = createMemo(() => {
    const drawn = svg();
    if (!drawn) {
      return "";
    }
    const address = URL.createObjectURL(new Blob([drawn], { type: "image/svg+xml" }));
    onCleanup(() => URL.revokeObjectURL(address));
    return address;
  });

  let viewer: HTMLElement | undefined;

  const show = (name: string) => {
    setChosen(name);
    setTab(FIRST);
    open(name);
    history.replaceState(null, "", `#${name}`);
    // On a narrow screen the list is the whole page and the diagram is under
    // it, so choosing one would otherwise change something out of sight.
    if (viewer && viewer.getBoundingClientRect().top > window.innerHeight / 2) {
      viewer.scrollIntoView({ behavior: "smooth", block: "start" });
    }
  };

  const shown = createMemo(() => TABS.filter((name) => name !== "SQL" || sql()));

  /// The command that would do, on the command line, what the controls above
  /// are doing here. The options are the same either way, and seeing them
  /// spelled out is the only way this page teaches them rather than just
  /// applying them.
  const command = createMemo(() => {
    const file = `examples/${chosen()}`;
    if (tab() === "SQL") {
      return `rusterd convert ${file}.sql --dialect ${dialect()}`;
    }
    return [`rusterd render ${file}.erd`, ...flagsFor(drawing())].join(" ");
  });

  return (
    <div class="workbench" style={styles.container}>
      <Header here="examples.html" language={LANGUAGE} />

      <p style={styles.blurb}>{say("everyExample", LANGUAGE)}</p>

      <div class="workbench-controls" style={styles.controls}>
        <Controls
          drawing={drawing()}
          change={(next) => {
            setDrawing(next);
            compile(erd());
          }}
          language={LANGUAGE}
        />
      </div>

      <pre class="workbench-command" style={styles.command}>{command()}</pre>

      <div class="workbench-layout">
        <label class="workbench-chooser">
          <select
            style={styles.chooserSelect}
            value={chosen()}
            onChange={(e) => show(e.currentTarget.value)}
          >
            {NAMES.map((name) => (
              <option value={name}>{name}</option>
            ))}
          </select>
          <Show when={ABOUT[chosen()]}>
            <span style={styles.entryAbout}>{ABOUT[chosen()][LANGUAGE]}</span>
          </Show>
        </label>

        <nav class="workbench-list">
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

        <section ref={viewer} class="workbench-viewer" style={styles.viewer}>
          <Tabs
            names={shown()}
            shown={tab()}
            choose={(name) => setTab(name as Tab)}
            label={(name) => (name === "Diagram" ? say("diagram", LANGUAGE) : name)}
            trailing={
              <span style={styles.aside}>
                <Show when={tab() === "Diagram"}>
                  <button style={styles.quiet} onClick={() => setActual(!actual())}>
                    {say(actual() ? "fit" : "actualSize", LANGUAGE)}
                  </button>
                  <a style={styles.quiet} href={alone()} target="_blank" rel="noopener noreferrer">
                    {say("openAlone", LANGUAGE)}
                  </a>
                </Show>
                <a
                  style={styles.quiet}
                  href={inRepo(`examples/${chosen()}.erd`)}
                  target="_blank"
                  rel="noopener noreferrer"
                >
                  {say("source", LANGUAGE)}
                </a>
              </span>
            }
          />

          {/* The Diagram tab is the result rather than something to act on, so
              it has nothing to put here and does not reserve the room. */}
          <Show when={tab() !== "Diagram" || edited()}>
            <div style={styles.toolbar}>
              <Show when={tab() === "SQL"}>
                <label style={control.field}>
                  <Named word={say("dialect", LANGUAGE)} option="dialect" />
                  <select
                    style={control.select}
                    value={dialect()}
                    onChange={(e) => setDialect(oneOf(DIALECTS, e.currentTarget.value, "auto"))}
                  >
                    {DIALECTS.map((entry) => (
                      <option value={entry.value}>{`${entry.value} — ${entry.label}`}</option>
                    ))}
                  </select>
                </label>
                <button style={styles.action} disabled={!ready()} onClick={convert}>
                  {say("fromSql", LANGUAGE)}
                </button>
              </Show>

              <Show when={tab() === "ERD"}>
                <button style={styles.action} disabled={!ready()} onClick={() => compile(erd())}>
                  {say("redraw", LANGUAGE)}
                </button>
                <span style={styles.hint}>{say("followsErd", LANGUAGE)}</span>
              </Show>

              <Show when={tab() === "SVG"}>
                <span style={styles.hint}>{say("followsSvg", LANGUAGE)}</span>
              </Show>

              <Show when={edited()}>
                <span style={styles.badge}>{say("edited", LANGUAGE)}</span>
                <button style={styles.revert} onClick={() => open(chosen())}>
                  {say("revert", LANGUAGE)}
                </button>
              </Show>
            </div>
          </Show>

          <Show when={error()}>
            <pre style={styles.error}>{error()}</pre>
          </Show>

          <Show when={tab() === "Diagram"}>
            <div
              class={
                actual() ? "diagram workbench-pane" : "diagram diagram-fit workbench-pane"
              }
              style={styles.drawing}
              innerHTML={svg()}
            />
          </Show>
          <Show when={tab() === "ERD"}>
            <textarea
              class="workbench-pane"
              style={styles.code}
              value={erd()}
              onInput={(e) => write(e.currentTarget.value)}
              spellcheck={false}
            />
          </Show>
          <Show when={tab() === "SVG"}>
            <textarea
              class="workbench-pane"
              style={styles.code}
              value={svg()}
              onInput={(e) => setSvg(e.currentTarget.value)}
              spellcheck={false}
            />
          </Show>
          <Show when={tab() === "SQL"}>
            <textarea
              class="workbench-pane"
              style={styles.code}
              value={sql()}
              onInput={(e) => setSql(e.currentTarget.value)}
              spellcheck={false}
            />
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
  controls: {
    display: "flex",
    "align-items": "center",
    "flex-wrap": "wrap",
    gap: "16px",
    "margin-bottom": "10px",
  },
  command: {
    "font-family": theme.mono,
    "font-size": "12.5px",
    color: theme.quiet,
    background: theme.panel,
    border: `1px solid ${theme.rule}`,
    "border-radius": "6px",
    padding: "8px 12px",
    margin: "0 0 16px",
    // A long line scrolls inside its own box rather than widening the page.
    "overflow-x": "auto",
    "white-space": "pre",
  },
  chooserSelect: {
    "font-family": theme.mono,
    "font-size": "14px",
    padding: "8px 10px",
    border: `1px solid ${theme.rule}`,
    "border-radius": "6px",
    background: theme.paper,
    color: theme.ink,
    width: "100%",
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
  toolbar: {
    display: "flex",
    "align-items": "center",
    "flex-wrap": "wrap",
    gap: "12px",
    padding: "12px 0",
    "min-height": "34px",
  },
  action: {
    "font-family": theme.sans,
    "font-size": "13px",
    "font-weight": "bold",
    padding: "6px 12px",
    border: `1px solid ${theme.rule}`,
    "border-radius": "4px",
    background: theme.panel,
    color: theme.ink,
    cursor: "pointer",
  },
  revert: {
    "font-family": theme.sans,
    "font-size": "13px",
    padding: "6px 12px",
    border: `1px solid ${theme.rule}`,
    "border-radius": "4px",
    background: theme.paper,
    color: theme.quiet,
    cursor: "pointer",
  },
  badge: {
    "font-size": "12px",
    color: theme.faint,
    // Sits apart from the buttons that act on the tab, at the end of the row.
    "margin-left": "auto",
  },
  hint: {
    "font-size": "13px",
    color: theme.faint,
  },
  aside: {
    display: "flex",
    "align-items": "center",
    gap: "16px",
  },
  quiet: {
    "font-family": theme.sans,
    "font-size": "13px",
    padding: "0",
    border: "none",
    background: "none",
    color: theme.quiet,
    "text-decoration": "none",
    cursor: "pointer",
  },
  error: {
    color: "#c00",
    "font-family": theme.mono,
    "font-size": "13px",
    padding: "12px",
    background: "#fee",
    "border-radius": "4px",
    margin: "0 0 12px",
    "white-space": "pre-wrap",
  },
  drawing: {
    border: `1px solid ${theme.rule}`,
    "border-radius": "6px",
  },
  code: {
    display: "block",
    width: "100%",
    "box-sizing": "border-box",
    "font-family": theme.mono,
    "font-size": "13px",
    "line-height": "1.55",
    border: `1px solid ${theme.rule}`,
    "border-radius": "6px",
    padding: "16px",
    background: theme.panel,
    color: theme.ink,
    margin: "0",
  },
} as const;

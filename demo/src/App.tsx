import { createSignal, onMount, Show } from "solid-js";
import init, { erdToSvg, sqlToErd } from "../../pkg/rusterd.js";

const DEFAULT_SQL = `-- The same schema as the ERD tab.
-- Relationships come from the foreign keys, so the "favorites" link and the
-- relationship labels of the ERD sample have no equivalent here.

CREATE TABLE Category (
    id INTEGER PRIMARY KEY,
    parent_id INTEGER REFERENCES Category(id),
    name VARCHAR(100) NOT NULL
);

CREATE TABLE User (
    id INTEGER PRIMARY KEY,
    email VARCHAR(255) NOT NULL UNIQUE,
    name VARCHAR(100),
    created_at TIMESTAMP
);

CREATE TABLE Product (
    id INTEGER PRIMARY KEY,
    category_id INTEGER REFERENCES Category(id),
    name VARCHAR(100) NOT NULL,
    price DECIMAL(10, 2),
    is_active BOOLEAN
);

CREATE TABLE "Order" (
    id INTEGER PRIMARY KEY,
    user_id INTEGER REFERENCES User(id),
    total DECIMAL(10, 2),
    status VARCHAR(20) NOT NULL
);`;

const DEFAULT_ERD = `# Sample ERD - demonstrates all features

# Grid-based layout
@hint.arrangement = {
    Category User;
    Product Order
}

# Self-referential entity
entity Category {
    id int pk
    parent_id int fk -> Category.id
    name string not null
}

entity User {
    id int pk
    email string unique not null
    name string
    created_at timestamp
}

entity Product {
    id int pk
    category_id int fk -> Category.id
    name string not null
    price decimal
    is_active boolean
}

entity Order {
    id int pk
    user_id int fk -> User.id
    total decimal
    status string not null
}

# All cardinality types: 1, *, 0..1, 1..*
rel {
    Category 1 -- * Category : "parent"
    Category 1 -- * Product
    User 1 -- * Order : "places"
    User 0..1 -- 1..* Product : "favorites"
}

# Filtered view
view simple {
    include User, Order
}`;

const TABS = ["SQL", "ERD", "SVG Code", "SVG Preview"] as const;
type Tab = (typeof TABS)[number];

const DETAIL_LEVELS = [
  { value: "all", label: "All columns" },
  { value: "pk_fk", label: "PK + FK only" },
  { value: "pk", label: "PK only" },
  { value: "tables", label: "Tables only" },
];

const DIALECTS = [
  { value: "auto", label: "Auto-detect" },
  { value: "postgres", label: "PostgreSQL" },
  { value: "mysql", label: "MySQL" },
  { value: "generic", label: "Generic" },
];

export default function App() {
  const [tab, setTab] = createSignal<Tab>("ERD");
  const [sql, setSql] = createSignal(DEFAULT_SQL);
  const [erd, setErd] = createSignal(DEFAULT_ERD);
  const [svg, setSvg] = createSignal("");
  const [dialect, setDialect] = createSignal("auto");
  const [detail, setDetail] = createSignal("all");
  const [error, setError] = createSignal("");
  const [ready, setReady] = createSignal(false);

  /// Run a conversion, showing whatever the compiler complains about.
  const convert = (step: () => Tab) => {
    try {
      setTab(step());
      setError("");
    } catch (e) {
      setError(String(e));
    }
  };

  /// The SQL parser skips statements it does not recognise, so a typo yields an
  /// empty schema rather than an error. Say so instead of showing a blank pane.
  const erdFromSql = () => {
    const converted = sqlToErd(sql(), dialect());
    if (!converted.trim()) {
      throw new Error("No tables found. Check the SQL, or choose the dialect explicitly.");
    }
    return converted;
  };

  const renderErd = (source: string) => {
    if (!source.trim()) {
      throw new Error("Nothing to render: the ERD is empty.");
    }
    return erdToSvg(source, null, detail());
  };

  const sqlToErdStep = () =>
    convert(() => {
      setErd(erdFromSql());
      return "ERD";
    });

  const sqlToSvgStep = () =>
    convert(() => {
      setSvg(renderErd(erdFromSql()));
      return "SVG Preview";
    });

  const erdToSvgStep = () =>
    convert(() => {
      setSvg(renderErd(erd()));
      return "SVG Preview";
    });

  onMount(async () => {
    await init();
    setReady(true);
    convert(() => {
      setSvg(renderErd(erd()));
      return "ERD";
    });
  });

  const detailSelect = () => (
    <label style={styles.field}>
      Detail
      <select
        style={styles.select}
        value={detail()}
        onChange={(e) => setDetail(e.currentTarget.value)}
      >
        {DETAIL_LEVELS.map((level) => (
          <option value={level.value}>{level.label}</option>
        ))}
      </select>
    </label>
  );

  return (
    <div style={styles.container}>
      <div style={styles.header}>
        <h1 style={styles.title}>Rusterd Demo</h1>
        <a
          style={styles.repoLink}
          href="https://github.com/misebox/rusterd"
          target="_blank"
          rel="noopener noreferrer"
        >
          GitHub
        </a>
      </div>

      <div style={styles.tabs}>
        {TABS.map((name) => (
          <button
            style={{ ...styles.tab, ...(tab() === name ? styles.tabActive : {}) }}
            onClick={() => setTab(name)}
          >
            {name}
          </button>
        ))}
      </div>

      <div style={styles.toolbar}>
        <Show when={tab() === "SQL"}>
          <label style={styles.field}>
            Dialect
            <select
              style={styles.select}
              value={dialect()}
              onChange={(e) => setDialect(e.currentTarget.value)}
            >
              {DIALECTS.map((d) => (
                <option value={d.value}>{d.label}</option>
              ))}
            </select>
          </label>
          {detailSelect()}
          <button style={styles.action} disabled={!ready()} onClick={sqlToErdStep}>
            SQL → ERD
          </button>
          <button style={styles.action} disabled={!ready()} onClick={sqlToSvgStep}>
            SQL → SVG
          </button>
        </Show>

        <Show when={tab() === "ERD"}>
          {detailSelect()}
          <button style={styles.action} disabled={!ready()} onClick={erdToSvgStep}>
            ERD → SVG
          </button>
        </Show>

        <Show when={tab() === "SVG Code"}>
          <span style={styles.hint}>Edits show up in SVG Preview as you type.</span>
        </Show>

        <Show when={tab() === "SVG Preview"}>
          <span style={styles.hint}>Rendered from the SVG Code tab.</span>
        </Show>
      </div>

      <Show when={error()}>
        <pre style={styles.error}>{error()}</pre>
      </Show>

      <div style={styles.panel}>
        <Show when={tab() === "SQL"}>
          <textarea
            style={styles.textarea}
            value={sql()}
            onInput={(e) => setSql(e.currentTarget.value)}
            spellcheck={false}
          />
        </Show>
        <Show when={tab() === "ERD"}>
          <textarea
            style={styles.textarea}
            value={erd()}
            onInput={(e) => setErd(e.currentTarget.value)}
            spellcheck={false}
          />
        </Show>
        <Show when={tab() === "SVG Code"}>
          <textarea
            style={styles.textarea}
            value={svg()}
            onInput={(e) => setSvg(e.currentTarget.value)}
            spellcheck={false}
          />
        </Show>
        <Show when={tab() === "SVG Preview"}>
          <div style={styles.preview} innerHTML={svg()} />
        </Show>
      </div>
    </div>
  );
}

const styles = {
  container: {
    "font-family": "system-ui, sans-serif",
    padding: "20px",
    "max-width": "1400px",
    margin: "0 auto",
    height: "100vh",
    "box-sizing": "border-box",
    display: "flex",
    "flex-direction": "column",
  },
  header: {
    display: "flex",
    "justify-content": "space-between",
    "align-items": "baseline",
    gap: "16px",
    "margin-bottom": "16px",
  },
  title: {
    margin: "0",
    color: "#333",
  },
  repoLink: {
    "font-size": "14px",
    color: "#555",
  },
  tabs: {
    display: "flex",
    gap: "4px",
    "border-bottom": "1px solid #ccc",
  },
  tab: {
    "font-family": "system-ui, sans-serif",
    "font-size": "14px",
    padding: "8px 14px",
    border: "1px solid transparent",
    "border-bottom": "none",
    "border-radius": "4px 4px 0 0",
    background: "transparent",
    color: "#555",
    cursor: "pointer",
  },
  tabActive: {
    border: "1px solid #ccc",
    "border-bottom": "1px solid #fff",
    "margin-bottom": "-1px",
    background: "#fff",
    color: "#111",
    "font-weight": "bold",
  },
  toolbar: {
    display: "flex",
    "align-items": "center",
    gap: "12px",
    padding: "12px 0",
    "min-height": "34px",
  },
  field: {
    display: "flex",
    "align-items": "center",
    gap: "6px",
    "font-size": "13px",
    color: "#666",
  },
  select: {
    "font-family": "system-ui, sans-serif",
    "font-size": "13px",
    padding: "4px 8px",
    border: "1px solid #ccc",
    "border-radius": "4px",
    background: "#fff",
  },
  action: {
    "font-family": "system-ui, sans-serif",
    "font-size": "13px",
    "font-weight": "bold",
    padding: "6px 12px",
    border: "1px solid #999",
    "border-radius": "4px",
    background: "#f6f6f6",
    color: "#222",
    cursor: "pointer",
  },
  hint: {
    "font-size": "13px",
    color: "#888",
  },
  panel: {
    flex: "1",
    display: "flex",
    "min-height": "0",
  },
  textarea: {
    flex: "1",
    "font-family": "monospace",
    "font-size": "14px",
    padding: "12px",
    border: "1px solid #ccc",
    "border-radius": "4px",
    resize: "none",
  },
  preview: {
    flex: "1",
    border: "1px solid #ccc",
    "border-radius": "4px",
    padding: "12px",
    overflow: "auto",
    background: "#fafafa",
  },
  error: {
    color: "#c00",
    "font-family": "monospace",
    "font-size": "13px",
    padding: "12px",
    background: "#fee",
    "border-radius": "4px",
    margin: "0 0 12px 0",
    "white-space": "pre-wrap",
  },
};

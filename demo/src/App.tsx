import { createSignal, createEffect, onMount } from "solid-js";
import init, { erdToSvg } from "../pkg/rusterd.js";

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

const DETAIL_LEVELS = [
  { value: "all", label: "All columns" },
  { value: "pk_fk", label: "PK + FK only" },
  { value: "pk", label: "PK only" },
  { value: "tables", label: "Tables only" },
];

export default function App() {
  const [source, setSource] = createSignal(DEFAULT_ERD);
  const [detail, setDetail] = createSignal("all");
  const [svg, setSvg] = createSignal("");
  const [error, setError] = createSignal("");
  const [ready, setReady] = createSignal(false);

  onMount(async () => {
    await init();
    setReady(true);
  });

  createEffect(() => {
    if (!ready()) return;
    try {
      const result = erdToSvg(source(), null, detail());
      setSvg(result);
      setError("");
    } catch (e) {
      setError(String(e));
      setSvg("");
    }
  });

  return (
    <div style={styles.container}>
      <h1 style={styles.title}>Rusterd Demo</h1>
      <div style={styles.main}>
        <div style={styles.editorPane}>
          <h2 style={styles.paneTitle}>ERD Source</h2>
          <textarea
            style={styles.textarea}
            value={source()}
            onInput={(e) => setSource(e.currentTarget.value)}
            spellcheck={false}
          />
        </div>
        <div style={styles.previewPane}>
          <div style={styles.previewHeader}>
            <h2 style={styles.paneTitle}>SVG Output</h2>
            <select
              style={styles.select}
              value={detail()}
              onChange={(e) => setDetail(e.currentTarget.value)}
            >
              {DETAIL_LEVELS.map((level) => (
                <option value={level.value}>{level.label}</option>
              ))}
            </select>
          </div>
          {error() ? (
            <pre style={styles.error}>{error()}</pre>
          ) : (
            <div style={styles.svgContainer} innerHTML={svg()} />
          )}
        </div>
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
  },
  title: {
    margin: "0 0 20px 0",
    color: "#333",
  },
  main: {
    display: "flex",
    gap: "20px",
    height: "calc(100vh - 120px)",
  },
  editorPane: {
    flex: "1",
    display: "flex",
    "flex-direction": "column",
  },
  previewPane: {
    flex: "1",
    display: "flex",
    "flex-direction": "column",
    overflow: "auto",
  },
  paneTitle: {
    margin: "0",
    "font-size": "14px",
    color: "#666",
  },
  previewHeader: {
    display: "flex",
    "justify-content": "space-between",
    "align-items": "center",
    "margin-bottom": "10px",
  },
  select: {
    "font-family": "system-ui, sans-serif",
    "font-size": "13px",
    padding: "4px 8px",
    border: "1px solid #ccc",
    "border-radius": "4px",
    background: "#fff",
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
  svgContainer: {
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
    margin: "0",
    "white-space": "pre-wrap",
  },
};

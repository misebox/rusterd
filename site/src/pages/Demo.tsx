import { createSignal, onMount, Show } from "solid-js";
import init, { erdToSvg, sqlToErd } from "../../../pkg/rusterd.js";
import Header from "../Header";
import Controls, { PLAIN } from "../Drawing";
import Tabs from "../Tabs";
import { language, say } from "../i18n";
import { theme } from "../theme";

const DEFAULT_SQL = `-- The same schema as the ERD tab: press "SQL → ERD" to regenerate it.
-- The converted ERD carries no placement: the layout works that out.
-- Order and Product are many-to-many through the OrderItem table.

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
    price DECIMAL(10, 2)
);

CREATE TABLE "Order" (
    id INTEGER PRIMARY KEY,
    user_id INTEGER REFERENCES User(id),
    status VARCHAR(20) NOT NULL,
    total DECIMAL(10, 2)
);

CREATE TABLE OrderItem (
    order_id INTEGER NOT NULL REFERENCES "Order"(id),
    product_id INTEGER NOT NULL REFERENCES Product(id),
    quantity INTEGER NOT NULL,
    PRIMARY KEY (order_id, product_id)
);`;

const DEFAULT_ERD = `# The same schema as the SQL tab, plus what SQL cannot express.
# Lines marked "ERD only" have no equivalent in a CREATE TABLE dump, so
# "SQL → ERD" will not produce them.

entity Category {
    id int pk
    parent_id int fk -> Category.id
    name varchar not null
}

entity User {
    id int pk
    email varchar unique not null
    name varchar
    created_at timestamp
}

entity Product {
    id int pk
    category_id int fk -> Category.id
    name varchar not null
    price decimal
}

entity Order {
    id int pk
    user_id int fk -> User.id
    status varchar not null
    total decimal
}

entity OrderItem {
    order_id int not null fk -> Order.id
    product_id int not null fk -> Product.id
    quantity int not null
    primary_key(order_id, product_id)
}

rel {
    # ERD only: relationship labels, and cardinalities beyond "1 -- *".
    # A foreign key alone cannot say "at most one parent" or "at least one item".
    Category 0..1 -- * Category : "parent"
    Category 1 -- * Product
    User 1 -- * Order : "places"
    Order 1 -- 1..* OrderItem : "contains"
    Product 1 -- * OrderItem
}

# Placement is worked out. Hints say what matters about the result, not where
# things go: keep these close, leave that out, draw that one as a name only.
@hint.near = { Order, OrderItem }
# @hint.omit  = { Category }
# @hint.brief = { OrderItem }

# ERD only: a named part of the schema.
# rusterd render schema.erd -f checkout
focus checkout {
    include User, Order, OrderItem
}`;

const TABS = ["SQL", "ERD", "SVG", "Diagram"] as const;
type Tab = (typeof TABS)[number];

const DIALECTS = [
  { value: "auto", label: "Auto-detect" },
  { value: "postgres", label: "PostgreSQL" },
  { value: "mysql", label: "MySQL" },
  { value: "generic", label: "Generic" },
];

export default function Demo() {
  const [tab, setTab] = createSignal<Tab>("ERD");
  const [sql, setSql] = createSignal(DEFAULT_SQL);
  const [erd, setErd] = createSignal(DEFAULT_ERD);
  const [svg, setSvg] = createSignal("");
  const [dialect, setDialect] = createSignal("auto");
  const [drawing, setDrawing] = createSignal(PLAIN);
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
    return erdToSvg(source, drawing());
  };

  const sqlToErdStep = () =>
    convert(() => {
      setErd(erdFromSql());
      return "ERD";
    });

  const sqlToSvgStep = () =>
    convert(() => {
      setSvg(renderErd(erdFromSql()));
      return "Diagram";
    });

  const erdToSvgStep = () =>
    convert(() => {
      setSvg(renderErd(erd()));
      return "Diagram";
    });

  /// Throw away hand edits to the SVG by rendering the ERD tab again.
  const resetSvgStep = () =>
    convert(() => {
      setSvg(renderErd(erd()));
      return "SVG";
    });

  onMount(async () => {
    await init();
    setReady(true);
    convert(() => {
      setSvg(renderErd(erd()));
      return "ERD";
    });
  });

  /// Both selects change how the diagram is drawn rather than what it contains,
  /// so they redraw it on the spot instead of waiting for a conversion button.
  /// The redraw comes from the ERD tab, which discards hand edits to the SVG.
  const redraw = () => {
    if (!ready()) return;
    convert(() => {
      setSvg(renderErd(erd()));
      return tab();
    });
  };

  const controls = () => (
    <Controls
      drawing={drawing()}
      change={(next) => {
        setDrawing(next);
        redraw();
      }}
      language={language()}
    />
  );

  return (
    <div style={styles.container}>
      <Header here="demo.html" language={language()} />

      <p style={styles.blurb}>{say("inYourBrowser", language())}</p>

      <Tabs
        names={TABS}
        shown={tab()}
        choose={(name) => setTab(name as Tab)}
        label={(name) => (name === "Diagram" ? say("diagram", language()) : name)}
      />

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
          {controls()}
          <button style={styles.action} disabled={!ready()} onClick={sqlToErdStep}>
            SQL → ERD
          </button>
          <button style={styles.action} disabled={!ready()} onClick={sqlToSvgStep}>
            SQL → SVG
          </button>
          <button style={styles.reset} onClick={() => setSql(DEFAULT_SQL)}>
            Reset
          </button>
        </Show>

        <Show when={tab() === "ERD"}>
          {controls()}
          <button style={styles.action} disabled={!ready()} onClick={erdToSvgStep}>
            ERD → SVG
          </button>
          <button style={styles.reset} onClick={() => setErd(DEFAULT_ERD)}>
            Reset
          </button>
        </Show>

        <Show when={tab() === "SVG"}>
          {controls()}
          <span style={styles.hint}>Edits show up in the Diagram tab as you type.</span>
          <button style={styles.reset} disabled={!ready()} onClick={resetSvgStep}>
            Reset
          </button>
        </Show>

        <Show when={tab() === "Diagram"}>
          {controls()}
          <span style={styles.hint}>Rendered from the SVG tab.</span>
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
        <Show when={tab() === "SVG"}>
          <textarea
            style={styles.textarea}
            value={svg()}
            onInput={(e) => setSvg(e.currentTarget.value)}
            spellcheck={false}
          />
        </Show>
        <Show when={tab() === "Diagram"}>
          <div style={styles.preview} innerHTML={svg()} />
        </Show>
      </div>
    </div>
  );
}

const styles = {
  container: {
    "font-family": theme.sans,
    padding: "0 20px 20px",
    "max-width": "1400px",
    margin: "0 auto",
    height: "100vh",
    "box-sizing": "border-box",
    display: "flex",
    "flex-direction": "column",
  },
  blurb: {
    "font-size": "14px",
    color: theme.faint,
    margin: "0 0 14px",
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
  reset: {
    "font-family": "system-ui, sans-serif",
    "font-size": "13px",
    padding: "6px 12px",
    border: "1px solid #ccc",
    "border-radius": "4px",
    background: "#fff",
    color: "#555",
    cursor: "pointer",
    // Sits apart from the conversion buttons, at the end of the toolbar.
    "margin-left": "auto",
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

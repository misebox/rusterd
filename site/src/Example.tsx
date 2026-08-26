import { createMemo, createSignal, Show } from "solid-js";
import { erdToSvg } from "../../pkg/rusterd.js";
import { Language, say } from "./i18n";
import Tabs from "./Tabs";
import { theme } from "./theme";

/// Lines an example is shown at before it starts scrolling.
const TALLEST = 18;

const SOURCE = "ERD";
const DRAWING = "Diagram";

/// One ERD example from a document, with what it compiles to.
///
/// `beside` puts the two next to each other, which suits a page with one
/// example on it. `tabs` stacks them behind a tab each, which suits a page that
/// is mostly examples. An example that draws nothing — a `rel` block on its
/// own, a hint — is only ever its source.
export default function Example(props: {
  source: string;
  layout: "beside" | "tabs";
  ready: boolean;
  language: Language;
}) {
  const [shown, setShown] = createSignal(SOURCE);

  const svg = createMemo(() => {
    if (!props.ready) {
      return "";
    }
    try {
      return erdToSvg(props.source);
    } catch {
      return "";
    }
  });

  // The stylesheet names that class too, so look for the element: an example
  // with no entities in it compiles happily to an empty diagram.
  const drawable = () => svg().includes('class="entity-border"');
  const beside = () => props.layout === "beside" && drawable();
  const showing = (panel: string) => beside() || !drawable() || shown() === panel;

  const source = () => (
    <textarea
      class="erd-source"
      readOnly
      spellcheck={false}
      style={{ "--lines": Math.min(props.source.split("\n").length, TALLEST) }}
      value={props.source}
    />
  );

  return (
    <figure style={styles.figure}>
      <Show when={props.layout === "tabs" && drawable()}>
        <Tabs
          names={[SOURCE, DRAWING]}
          shown={shown()}
          choose={setShown}
          label={(name) => (name === DRAWING ? say("diagram", props.language) : name)}
        />
      </Show>

      <div style={beside() ? styles.beside : {}}>
        <Show when={showing(SOURCE)}>{source()}</Show>
        <Show when={showing(DRAWING) && drawable()}>
          <div class="diagram" style={styles.drawing} innerHTML={svg()} />
        </Show>
      </div>
    </figure>
  );
}

const styles = {
  figure: {
    margin: "1.2em 0",
  },
  beside: {
    display: "grid",
    "grid-template-columns": "minmax(0, 1fr) minmax(0, 1fr)",
    gap: "16px",
    "align-items": "start",
  },
  drawing: {
    padding: "12px",
    border: `1px solid ${theme.rule}`,
    "border-radius": "6px",
    background: theme.paper,
    "overflow-x": "auto",
  },
} as const;

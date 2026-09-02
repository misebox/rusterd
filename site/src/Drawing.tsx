import type { Detail, Notation } from "../../pkg/rusterd.js";
import { Language } from "./i18n";
import { theme } from "./theme";

/// Everything about how a diagram is drawn rather than what is in it — the
/// same set the compiler takes as `DrawOptions`.
export type Drawing = {
  detail: Detail;
  notation: Notation;
  aspect: string;
  legend: boolean;
  dense: boolean;
};

export const PLAIN: Drawing = {
  detail: "all",
  notation: "crowsfoot",
  aspect: "1:1",
  legend: false,
  dense: false,
};

/// The words for the controls and their choices. These belong to the controls
/// rather than to the site, so they live beside them.
const WORDS = {
  detail: { en: "Detail", ja: "詳細度" },
  notation: { en: "Notation", ja: "記法" },
  aspect: { en: "Shape", ja: "形" },
  dense: { en: "Dense", ja: "密" },
  legend: { en: "Legend", ja: "凡例" },
} as const;

type Choice<T extends string> = { value: T; en: string; ja: string };

/// The shapes worth offering: a screen, a slide, a sheet of paper either way
/// up. Any `width:height` is accepted by the compiler; these are the ones a
/// diagram is usually going into.
const ASPECT: Choice<string>[] = [
  { value: "1:1", en: "Square — a screen", ja: "正方形 — 画面" },
  { value: "16:9", en: "16:9 — a slide", ja: "16:9 — スライド" },
  { value: "297:210", en: "A4 landscape", ja: "A4 横" },
  { value: "210:297", en: "A4 portrait", ja: "A4 縦" },
];

const DETAIL: Choice<Detail>[] = [
  { value: "all", en: "All columns", ja: "全列" },
  { value: "pk_fk", en: "PK + FK only", ja: "主キーと外部キー" },
  { value: "pk", en: "PK only", ja: "主キーのみ" },
  { value: "tables", en: "Tables only", ja: "表名のみ" },
];

const NOTATION: Choice<Notation>[] = [
  { value: "crowsfoot", en: "Crow's foot symbols", ja: "クロウズフット" },
  { value: "text", en: "Text — 1, 0..1, ✱, 1..✱", ja: "文字 — 1, 0..1, ✱, 1..✱" },
];

/// A `<select>` reports its value as a string, but the compiler accepts only
/// the names it published. Finding the value back in the list it was offered
/// from is what makes it one of those names again.
export function oneOf<T extends string>(
  choices: readonly { value: T }[],
  value: string,
  fallback: T,
): T {
  return choices.find((choice) => choice.value === value)?.value ?? fallback;
}

/// The controls for how to draw a diagram, wherever one is being drawn.
export default function Controls(props: {
  drawing: Drawing;
  change: (drawing: Drawing) => void;
  language: Language;
}) {
  const set = (part: Partial<Drawing>) => props.change({ ...props.drawing, ...part });
  const word = (key: keyof typeof WORDS) => WORDS[key][props.language];

  return (
    <>
      <label style={styles.field}>
        {word("detail")}
        <select
          style={styles.select}
          value={props.drawing.detail}
          onChange={(e) => set({ detail: oneOf(DETAIL, e.currentTarget.value, PLAIN.detail) })}
        >
          {DETAIL.map((level) => (
            <option value={level.value}>{level[props.language]}</option>
          ))}
        </select>
      </label>

      <label style={styles.field}>
        {word("notation")}
        <select
          style={styles.select}
          value={props.drawing.notation}
          onChange={(e) =>
            set({ notation: oneOf(NOTATION, e.currentTarget.value, PLAIN.notation) })
          }
        >
          {NOTATION.map((entry) => (
            <option value={entry.value}>{entry[props.language]}</option>
          ))}
        </select>
      </label>

      <label style={styles.field}>
        {word("aspect")}
        <select
          style={styles.select}
          value={props.drawing.aspect}
          onChange={(e) => set({ aspect: oneOf(ASPECT, e.currentTarget.value, PLAIN.aspect) })}
        >
          {ASPECT.map((shape) => (
            <option value={shape.value}>{shape[props.language]}</option>
          ))}
        </select>
      </label>

      <label style={styles.field}>
        <input
          type="checkbox"
          checked={props.drawing.dense}
          onChange={(e) => set({ dense: e.currentTarget.checked })}
        />
        {word("dense")}
      </label>

      <label style={styles.field}>
        <input
          type="checkbox"
          checked={props.drawing.legend}
          onChange={(e) => set({ legend: e.currentTarget.checked })}
        />
        {word("legend")}
      </label>
    </>
  );
}

/// The examples page puts its dialect picker in the same toolbar as these,
/// where it would look like a stray unless it is dressed the same.
export const styles = {
  field: {
    display: "flex",
    "align-items": "center",
    gap: "6px",
    "font-size": "13px",
    color: theme.quiet,
    "white-space": "nowrap",
  },
  select: {
    "font-family": theme.sans,
    "font-size": "13px",
    padding: "4px 8px",
    border: `1px solid ${theme.rule}`,
    "border-radius": "4px",
    background: theme.paper,
  },
} as const;

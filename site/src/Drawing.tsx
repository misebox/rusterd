import { Language } from "./i18n";
import { theme } from "./theme";

/// Everything about how a diagram is drawn rather than what is in it — the
/// same set the compiler takes as `DrawOptions`.
export type Drawing = {
  detail: string;
  notation: string;
  legend: boolean;
  dense: boolean;
};

export const PLAIN: Drawing = {
  detail: "all",
  notation: "crowsfoot",
  legend: false,
  dense: false,
};

/// The words for the controls and their choices. These belong to the controls
/// rather than to the site, so they live beside them.
const WORDS = {
  detail: { en: "Detail", ja: "詳細度" },
  notation: { en: "Notation", ja: "記法" },
  dense: { en: "Dense", ja: "密" },
  legend: { en: "Legend", ja: "凡例" },
} as const;

const DETAIL = [
  { value: "all", en: "All columns", ja: "全列" },
  { value: "pk_fk", en: "PK + FK only", ja: "主キーと外部キー" },
  { value: "pk", en: "PK only", ja: "主キーのみ" },
  { value: "tables", en: "Tables only", ja: "表名のみ" },
];

const NOTATION = [
  { value: "crowsfoot", en: "Crow's foot symbols", ja: "クロウズフット" },
  { value: "text", en: "Text — 1, 0..1, ✱, 1..✱", ja: "文字 — 1, 0..1, ✱, 1..✱" },
];

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
          onChange={(e) => set({ detail: e.currentTarget.value })}
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
          onChange={(e) => set({ notation: e.currentTarget.value })}
        >
          {NOTATION.map((entry) => (
            <option value={entry.value}>{entry[props.language]}</option>
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

const styles = {
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

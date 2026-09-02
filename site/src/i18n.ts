/// Which language the site is being read in.
///
/// The address decides, so a page can be linked to in either language; what was
/// chosen last is remembered for the next visit. English is the fallback
/// because the documents are written in it: a translation that has not been
/// written yet leaves the original showing rather than a gap.

export type Language = "en" | "ja";

export const LANGUAGES: { code: Language; name: string }[] = [
  { code: "en", name: "English" },
  { code: "ja", name: "日本語" },
];

const REMEMBERED = "rusterd.language";

export function language(): Language {
  const asked = new URLSearchParams(location.search).get("lang");
  if (known(asked)) {
    return asked;
  }
  try {
    const last = localStorage.getItem(REMEMBERED);
    if (known(last)) {
      return last;
    }
  } catch {
    // Storage can be refused; the address and the default still work.
  }
  return navigator.language.startsWith("ja") ? "ja" : "en";
}

export function choose(code: Language) {
  try {
    localStorage.setItem(REMEMBERED, code);
  } catch {
    // Then it is only remembered for as long as the address says so.
  }
  const url = new URL(location.href);
  url.searchParams.set("lang", code);
  location.href = url.toString();
}

/// Tell the browser which language the page turned out to be in, for the sake
/// of hyphenation, spell checking and anything reading it aloud.
export function markPage(code: Language) {
  document.documentElement.lang = code;
}

/// The address of another page in the language being read.
export function inLanguage(href: string, code: Language): string {
  return code === "en" ? href : `${href}?lang=${code}`;
}

function known(code: string | null): code is Language {
  return LANGUAGES.some((entry) => entry.code === code);
}

/// The words the site says for itself. What the documents say is in the
/// documents.
const WORDS = {
  overview: { en: "Overview", ja: "概要" },
  start: { en: "Get started", ja: "はじめに" },
  languagePage: { en: "Language", ja: "言語仕様" },
  examples: { en: "Examples", ja: "サンプル" },

  getStarted: { en: "Get started", ja: "はじめる" },
  tryIt: { en: "Try it in the browser", ja: "ブラウザで試す" },

  forModels: {
    en: "For a model writing .erd files",
    ja: ".erd を書くモデルに渡すもの",
  },
  compiledHere: {
    en: "Every diagram on this page was compiled in your browser, by the same code the command line runs.",
    ja: "このページの図はすべて、コマンドラインと同じコードがブラウザ上でコンパイルしたものです。",
  },
  untranslated: {
    en: "This page has not been translated yet, and is shown in English.",
    ja: "この文書はまだ翻訳されていないため、英語のまま表示しています。",
  },

  everyExample: {
    en: "Every file in examples/, compiled here in the browser by the same code the command line runs. Edit any of them and the diagram follows.",
    ja: "examples/ にあるファイルを、コマンドラインと同じコードでこの場でコンパイルしたものです。書き換えれば図もついてきます。",
  },
  source: { en: "Source", ja: "ソース" },
  actualSize: { en: "Actual size", ja: "実寸" },
  fit: { en: "Fit", ja: "全体" },
  openAlone: { en: "Open on its own", ja: "別タブで開く" },
  revert: { en: "Revert", ja: "戻す" },
  edited: { en: "edited", ja: "編集済み" },
  fromSql: { en: "SQL → ERD", ja: "SQL → ERD" },
  redraw: { en: "ERD → Diagram", ja: "ERD → 図" },
  followsErd: { en: "Redrawn from the ERD tab as you type.", ja: "ERD タブの内容から描き直しています。" },
  followsSvg: { en: "Drawn from the SVG tab as you type.", ja: "SVG タブの内容をそのまま描いています。" },
  inYourBrowser: {
    en: "Nothing here is sent anywhere: the compiler is a WebAssembly build of the same Rust the command line runs, and it compiles as you type.",
    ja: "何もサーバには送られません。コマンドラインと同じ Rust を WebAssembly にビルドしたコンパイラが、この場で変換しています。",
  },
  diagram: { en: "Diagram", ja: "図" },
} as const;

export function say(key: keyof typeof WORDS, code: Language): string {
  return WORDS[key][code];
}

import { onMount } from "solid-js";
import { choose, inLanguage, Language, LANGUAGES, markPage, say } from "./i18n";
import { theme } from "./theme";

const PAGES = [
  { href: "index.html", word: "overview" },
  { href: "start.html", word: "start" },
  { href: "language.html", word: "languagePage" },
  { href: "examples.html", word: "examples" },
  { href: "demo.html", word: "demo" },
] as const;

/// The same masthead on every page. `here` is the file name of the page drawing
/// it, so that page's own link reads as a heading rather than as somewhere to
/// go.
export default function Header(props: { here: string; language: Language }) {
  onMount(() => markPage(props.language));

  return (
    <header style={styles.bar}>
      <a href={inLanguage("index.html", props.language)} style={styles.name}>
        rusterd
      </a>
      <nav style={styles.nav}>
        {PAGES.map((entry) => (
          <a
            href={inLanguage(entry.href, props.language)}
            style={{
              ...styles.link,
              ...(entry.href === props.here ? styles.current : {}),
            }}
          >
            {say(entry.word, props.language)}
          </a>
        ))}
        <a
          href="https://github.com/misebox/rusterd"
          target="_blank"
          rel="noopener noreferrer"
          style={styles.link}
        >
          GitHub
        </a>
        <span style={styles.languages}>
          {LANGUAGES.map((entry) => (
            <button
              style={{
                ...styles.language,
                ...(entry.code === props.language ? styles.current : {}),
              }}
              onClick={() => choose(entry.code)}
            >
              {entry.name}
            </button>
          ))}
        </span>
      </nav>
    </header>
  );
}

const styles = {
  bar: {
    display: "flex",
    "flex-direction": "column",
    "align-items": "center",
    gap: "10px",
    padding: "22px 0 16px",
    "border-bottom": `1px solid ${theme.rule}`,
    "margin-bottom": "28px",
  },
  name: {
    "font-family": theme.mono,
    "font-size": "22px",
    "font-weight": "bold",
    color: theme.ink,
    "text-decoration": "none",
    "letter-spacing": "0.02em",
  },
  nav: {
    display: "flex",
    "flex-wrap": "wrap",
    "justify-content": "center",
    "align-items": "center",
    gap: "20px",
    "font-size": "14px",
  },
  link: {
    color: theme.quiet,
    "text-decoration": "none",
  },
  current: {
    color: theme.ink,
    "font-weight": "bold",
  },
  languages: {
    display: "flex",
    gap: "10px",
    "padding-left": "20px",
    "border-left": `1px solid ${theme.rule}`,
  },
  language: {
    "font-family": theme.sans,
    "font-size": "13px",
    padding: "0",
    border: "none",
    background: "none",
    color: theme.quiet,
    cursor: "pointer",
  },
} as const;

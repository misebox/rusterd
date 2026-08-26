import { theme } from "./theme";

const PAGES = [
  { href: "index.html", label: "Overview" },
  { href: "start.html", label: "Get started" },
  { href: "language.html", label: "Language" },
  { href: "examples.html", label: "Examples" },
  { href: "demo.html", label: "Demo" },
];

/// The same masthead on every page. `here` is the file name of the page drawing
/// it, so that page's own link reads as a heading rather than as somewhere to
/// go.
export default function Header(props: { here: string }) {
  return (
    <header style={styles.bar}>
      <a href="index.html" style={styles.name}>
        rusterd
      </a>
      <nav style={styles.nav}>
        {PAGES.map((entry) => (
          <a
            href={entry.href}
            style={{
              ...styles.link,
              ...(entry.href === props.here ? styles.current : {}),
            }}
          >
            {entry.label}
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
} as const;

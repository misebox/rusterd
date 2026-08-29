import { marked } from "marked";
import { For } from "solid-js";
import Example from "./Example";
import { Language } from "./i18n";
import { inRepo } from "./project";

// Headings carry their own name, so a link to one lands on it. marked stopped
// doing this itself, and the documents link to their own sections.
marked.use({
  renderer: {
    heading({ tokens, depth }) {
      const text = this.parser.parseInline(tokens);
      const name = text
        .replace(/<[^>]*>/g, "")
        .trim()
        .toLowerCase()
        .replace(/[^\p{Letter}\p{Number}]+/gu, "-")
        .replace(/^-|-$/g, "");
      return `<h${depth} id="${name}">${text}</h${depth}>\n`;
    },
  },
});

/// Where a link in the documents should go once it is on the site rather than
/// in the repository.
const ELSEWHERE: Record<string, string> = {
  "docs/DSL-spec.md": "language.html",
  "DSL-spec.md": "language.html",
  "../README.md#options": "start.html#options",
};

/// A link to an example file goes to the page that shows it compiled.
const EXAMPLE = /^(?:\.\.\/)?examples\/(.+)\.erd$/;

/// Files the site serves itself, so the link hands over the grammar rather than
/// a page about it.
const FILES: Record<string, string> = {
  "erd.gbnf": "erd.gbnf",
  "docs/erd.gbnf": "erd.gbnf",
  "sql.gbnf": "sql.gbnf",
  "docs/sql.gbnf": "sql.gbnf",
};

/// Render one of the project's markdown documents.
///
/// The documents are the same files the repository ships — nothing is restated
/// here, so there is nothing to keep in step. What the page adds is that every
/// ERD example in them is compiled, which is the one thing a page can do that a
/// file cannot.
export default function Markdown(props: {
  source: string;
  layout: "beside" | "tabs";
  ready: boolean;
  language: Language;
}) {
  return (
    <For each={parts(props.source)}>
      {(part) =>
        part.erd === undefined ? (
          <div class="prose" innerHTML={marked.parse(reroute(part.prose), { async: false })} />
        ) : (
          <Example
            source={part.erd}
            layout={props.layout}
            ready={props.ready}
            language={props.language}
          />
        )
      }
    </For>
  );
}

/// A document, cut at its ERD examples: prose the page renders as markdown, and
/// sources the page compiles.
function parts(document: string): { prose: string; erd?: string }[] {
  const found: { prose: string; erd?: string }[] = [];
  const fence = /```erd\n([\s\S]*?)```/g;
  let read = 0;

  for (let block = fence.exec(document); block; block = fence.exec(document)) {
    found.push({ prose: document.slice(read, block.index) });
    found.push({ prose: "", erd: block[1].trimEnd() });
    read = block.index + block[0].length;
  }
  found.push({ prose: document.slice(read) });

  return found.filter((part) => part.erd !== undefined || part.prose.trim());
}

/// Point the documents' relative links at the site, at a file the site serves,
/// or at the repository — and drop the pictures.
///
/// A file has to link to a picture of what an example compiles to. A page
/// compiles it, so the picture would be the same diagram a second time.
function reroute(prose: string): string {
  return prose
    .replace(/^!\[[^\]]*\]\([^)]*\.svg\)\s*$/gm, "")
    .replace(/\]\(([^)]+)\)/g, (whole, href: string) => {
      if (/^[a-z]+:|^#/.test(href)) {
        return whole;
      }
      const file = FILES[href];
      if (file) {
        return `](${file})`;
      }
      const example = href.match(EXAMPLE);
      if (example) {
        return `](examples.html#${example[1]})`;
      }
      return `](${ELSEWHERE[href] ?? inRepo(href.replace(/^\.\//, ""))})`;
    });
}

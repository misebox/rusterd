import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import type { Plugin } from "vite";
import { sections } from "./src/document";

/// Where the built site lives, for the links inside the files it serves. A
/// model is handed a URL, not a directory, so relative paths are no use to it.
const SITE = "https://misebox.github.io/rusterd";
const REPO = "https://github.com/misebox/rusterd";

const read = (path: string) =>
  readFileSync(fileURLToPath(new URL(`../${path}`, import.meta.url)), "utf8");

/// The plain-text files a model is pointed at, each at an address that does not
/// change: `llms.txt` says what is here, `llms-full.txt` is the whole of what a
/// model needs to write `.erd` files, and the grammars constrain it while it
/// does. The site is generated from the same documents, so none of this is a
/// second copy to keep in step.
function forModels(): Record<string, string> {
  const overview = read("README.md");
  const reference = read("docs/DSL-spec.md");

  return {
    "erd.gbnf": read("docs/erd.gbnf"),
    "sql.gbnf": read("docs/sql.gbnf"),
    "llms.txt": index(),
    "llms-full.txt": whole(reference, sections(overview, ["Options"])),
  };
}

function index(): string {
  return `# rusterd

> An ER diagram DSL that compiles to SVG. Entities and relationships are
> written in a text file; where anything goes on the page is worked out by the
> compiler. Written in Rust, with a WebAssembly build that runs in a browser.

## Writing .erd files

- [llms-full.txt](${SITE}/llms-full.txt): the language as the parser accepts
  it, and the options it is rendered with. Everything needed to author a file,
  in one fetch.
- [erd.gbnf](${SITE}/erd.gbnf): the same language as a GBNF grammar, for
  constrained decoding. Stricter than the parser, so everything it produces
  parses.
- [sql.gbnf](${SITE}/sql.gbnf): the DDL subset \`rusterd convert\` reads.

## Elsewhere

- [Documentation](${SITE}/): the same material as pages, with every example
  compiled in the browser.
- [Repository](${REPO})
`;
}

function whole(reference: string, options: string): string {
  return [
    "# rusterd: writing .erd files",
    "",
    "Two documents, end to end: the language reference, then the options it is",
    "rendered with. Both are the files the repository ships.",
    "",
    "---",
    "",
    // The options are the second half of this file, not a document elsewhere.
    absolute(reference.replace("](../README.md#options)", "](#options)")),
    "",
    "---",
    "",
    absolute(options),
  ].join("\n");
}

/// A link that made sense inside the repository, made sense of on its own.
function absolute(document: string): string {
  return document.replace(/\]\(([^)]+)\)/g, (whole, href: string) => {
    if (/^[a-z]+:|^#/.test(href)) {
      return whole;
    }
    const file = href.replace(/^(?:\.\.\/)?(?:docs\/)?/, "");
    if (file === "erd.gbnf" || file === "sql.gbnf") {
      return `](${SITE}/${file})`;
    }
    return `](${REPO}/blob/main/${href.replace(/^\.\.\//, "")})`;
  });
}

/// Serve those files in development too, so a link to one is not a link to a
/// page that only exists after a build.
export default function llms(): Plugin {
  return {
    name: "rusterd-llms",
    configureServer(server) {
      server.middlewares.use((request, response, next) => {
        const asked = (request.url ?? "").replace(/^\/|\?.*$/g, "");
        const body = forModels()[asked];
        if (body === undefined) {
          return next();
        }
        response.setHeader("Content-Type", "text/plain; charset=utf-8");
        response.end(body);
      });
    },
    generateBundle() {
      for (const [fileName, source] of Object.entries(forModels())) {
        this.emitFile({ type: "asset", fileName, source });
      }
    },
  };
}

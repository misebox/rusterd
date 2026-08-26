import { marked } from "marked";
import { createEffect, onMount } from "solid-js";
import { erdToSvg } from "../../pkg/rusterd.js";
import erdGrammar from "../../docs/erd.gbnf?url";
import sqlGrammar from "../../docs/sql.gbnf?url";
const REPO = "https://github.com/misebox/rusterd/blob/main";

/// Where a link in the documents should go once it is on the site rather than
/// in the repository.
const ELSEWHERE: Record<string, string> = {
  "docs/DSL-spec.md": "language.html",
  "DSL-spec.md": "language.html",
};

/// Files the site serves itself, so the link hands over the grammar rather than
/// a page about it.
const FILES: Record<string, string> = {
  "erd.gbnf": erdGrammar,
  "docs/erd.gbnf": erdGrammar,
  "sql.gbnf": sqlGrammar,
  "docs/sql.gbnf": sqlGrammar,
};

/// Render one of the project's markdown documents.
///
/// The documents are the same files the repository ships — nothing is restated
/// here, so there is nothing to keep in step. What the page adds is that every
/// ERD example in them is compiled and drawn beside its source, which is the
/// one thing a page can do that a file cannot.
export default function Markdown(props: { source: string; ready: boolean }) {
  let host!: HTMLDivElement;

  onMount(() => {
    host.innerHTML = marked.parse(props.source, { async: false }) as string;
    reroute(host);
  });

  // Drawing has to wait for the compiler to finish loading.
  createEffect(() => {
    if (props.ready) {
      illustrate(host);
    }
  });

  return <div ref={host} class="prose" />;
}

/// Point the documents' relative links at the site or at the repository.
function reroute(host: HTMLElement) {
  for (const link of host.querySelectorAll("a[href]")) {
    const href = link.getAttribute("href") ?? "";
    if (/^[a-z]+:|^#|\.html$/.test(href)) {
      continue;
    }
    const file = FILES[href];
    if (file) {
      link.setAttribute("href", file);
      link.setAttribute("download", href.split("/").pop() ?? "");
      continue;
    }

    const known = ELSEWHERE[href];
    link.setAttribute("href", known ?? `${REPO}/${href.replace(/^\.\//, "")}`);
    if (!known) {
      link.setAttribute("target", "_blank");
      link.setAttribute("rel", "noopener noreferrer");
    }
  }
}

/// Compile every ERD example and put the diagram beside its source.
///
/// Some examples are fragments — a `rel` block on its own, a hint — which
/// compile to nothing. Those are left as they are rather than shown next to an
/// empty box.
function illustrate(host: HTMLElement) {
  for (const block of host.querySelectorAll("code.language-erd")) {
    const pre = block.parentElement;
    if (!pre || pre.dataset.drawn) {
      continue;
    }
    pre.dataset.drawn = "yes";

    let svg: string;
    try {
      svg = erdToSvg(block.textContent ?? "", null, null, null, null, null);
    } catch {
      continue;
    }
    if (!svg.includes("entity-border")) {
      continue;
    }

    const figure = document.createElement("figure");
    figure.className = "erd-example";
    pre.replaceWith(figure);
    figure.append(pre);

    const drawing = document.createElement("div");
    drawing.className = "erd-drawing diagram";
    drawing.innerHTML = svg;
    figure.append(drawing);
  }
}

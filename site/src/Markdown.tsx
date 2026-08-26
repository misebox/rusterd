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
    undraw(host);
    box(host);
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

/// Drop the diagrams the documents point at.
///
/// A file has to link to a picture of what an example compiles to. A page
/// compiles it, right beside the source — so the picture would be the same
/// diagram a second time.
function undraw(host: HTMLElement) {
  for (const image of host.querySelectorAll('img[src$=".svg"]')) {
    image.parentElement?.remove();
  }
}

/// Put every ERD example in a box of its own, so that a long schema takes as
/// much of the page as a short one and scrolls the rest.
///
/// A read-only field rather than the block markdown produced: it is the
/// familiar way to hand over text that can be selected and copied but not
/// edited, and it comes with the scrollbar.
function box(host: HTMLElement) {
  for (const block of host.querySelectorAll("code.language-erd")) {
    const pre = block.parentElement;
    if (!pre || pre.dataset.boxed) {
      continue;
    }

    const source = block.textContent ?? "";
    const field = document.createElement("textarea");
    field.className = "erd-source";
    field.readOnly = true;
    field.spellcheck = false;
    field.value = source;
    field.rows = Math.min(source.trimEnd().split("\n").length, TALLEST);

    const figure = document.createElement("figure");
    figure.className = "erd-example";
    figure.dataset.source = source;
    pre.replaceWith(figure);
    figure.append(field);
  }
}

/// Compile each example and put the diagram beside its source.
///
/// Some examples are fragments — a `rel` block on its own, a hint — which
/// compile to nothing. Those are left as text alone rather than shown beside an
/// empty box.
function illustrate(host: HTMLElement) {
  for (const figure of host.querySelectorAll<HTMLElement>("figure.erd-example")) {
    if (figure.dataset.drawn) {
      continue;
    }
    figure.dataset.drawn = "yes";

    let svg: string;
    try {
      svg = erdToSvg(figure.dataset.source ?? "");
    } catch {
      continue;
    }
    if (!svg.includes("entity-border")) {
      figure.classList.add("erd-example-alone");
      continue;
    }

    const drawing = document.createElement("div");
    drawing.className = "erd-drawing diagram";
    drawing.innerHTML = svg;
    figure.append(drawing);
  }
}

/// Lines an example is shown at before it starts scrolling.
const TALLEST = 18;

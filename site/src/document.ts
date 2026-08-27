/// Taking a page's worth out of one of the project's documents.
///
/// The documents are written to be read whole, in the repository. A page is
/// read for one thing at a time, so it takes the sections it is about — by the
/// headings they are written under, which is the only handle a markdown file
/// offers.

/// The first paragraph after the title: what the thing is, in a sentence.
/// Whatever follows it — where to find the site, what it is licensed under —
/// is for a reader of the file rather than of the site.
export function intro(document: string): string {
  const body = document.replace(/^#\s+.*\n/, "").trim();
  return body.split(/\n{2,}/).find(sentences)?.trim() ?? "";
}

/// Whether a paragraph says anything. A row of badges is a paragraph like any
/// other, and belongs under the title of a file rather than at the head of a
/// page: it is pictures and links and not a sentence.
function sentences(paragraph: string): boolean {
  return paragraph.replace(/\[?!\[[^\]]*\]\([^)]*\)\]?(\([^)]*\))?/g, "").trim() !== "";
}

/// The named sections, in the order asked for. A heading that is not there is
/// said so on the page rather than quietly left out, because the only way that
/// happens is a document being reorganised without the site following.
export function sections(document: string, headings: string[]): string {
  return headings
    .map((heading) => {
      const start = index(document, `\n## ${heading}\n`);
      if (start === document.length) {
        return `## ${heading}\n\nThis section is no longer in the document it is read from.\n`;
      }
      const rest = document.slice(start + 1);
      return rest.slice(0, index(rest, "\n## ")).trim();
    })
    .join("\n\n");
}

/// A document cut at its `## ` headings, in the order they are written.
export function chunks(document: string): string[] {
  return document
    .split(/\n(?=## )/)
    .map((chunk) => chunk.trim())
    .filter(Boolean);
}

/// A section with its heading taken off, for a page that shows what it is
/// rather than saying it — an example under a word reading "Example".
export function withoutHeading(section: string): string {
  return section.replace(/^##\s+.*\n/, "").trim();
}

/// Where `needle` starts, or the end of the haystack when it is not in it.
function index(haystack: string, needle: string): number {
  const at = haystack.indexOf(needle);
  return at === -1 ? haystack.length : at;
}

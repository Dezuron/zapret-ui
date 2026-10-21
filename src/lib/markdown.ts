import MarkdownIt from "markdown-it";

// Changelog/release notes are authored by us, but HTML is still disabled so a
// compromised GitHub release body cannot inject markup into the webview.
const markdown = new MarkdownIt({
  html: false,
  linkify: false,
  breaks: true,
});

export function renderMarkdown(source: string): string {
  return markdown.render(source ?? "");
}

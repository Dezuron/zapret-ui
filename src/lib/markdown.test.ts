import { describe, expect, it } from "vitest";
import { renderMarkdown } from "./markdown";

describe("renderMarkdown", () => {
  it("renders headings, lists and bold text", () => {
    const html = renderMarkdown("### Добавлено\n\n- **Важное** изменение");
    expect(html).toContain("<h3>Добавлено</h3>");
    expect(html).toContain("<ul>");
    expect(html).toContain("<strong>Важное</strong>");
  });

  it("does not execute raw HTML", () => {
    const html = renderMarkdown("<script>alert('xss')</script>");
    expect(html).not.toContain("<script>");
    expect(html).toContain("&lt;script&gt;");
  });
});

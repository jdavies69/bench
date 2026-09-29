import { describe, expect, it } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { Markdown } from "./Markdown";

describe("conversation Markdown", () => {
  it("renders structured content without exposing formatting markers", () => {
    const result = renderToStaticMarkup(<Markdown text={'# Title\n\n**Bold** and [a link](https://example.com).\n\n- First\n- Second\n\n```ts\nconst value = 1;\n```\n\n| Name | Value |\n| --- | --- |\n| A | B |'} />);
    expect(result).toContain('<h1>Title</h1>');
    expect(result).toContain('<strong>Bold</strong>');
    expect(result).toContain('<ul>');
    expect(result).toContain('<pre>');
    expect(result).toContain('<table>');
    expect(result).toContain('rel="noopener noreferrer"');
    expect(result).not.toContain('**Bold**');
  });

  it("does not execute provider-supplied HTML or javascript links", () => {
    const result = renderToStaticMarkup(<Markdown text={'<script>alert(1)</script>\n\n[click](javascript:alert(1))'} />);
    expect(result).not.toContain('<script>');
    expect(result).not.toContain('href="javascript:');
  });
});

// @vitest-environment jsdom
import { describe, expect, it } from "vitest";
import { buildPreviewDocument, safePreviewHtml } from "./preview";

describe("isolated Website document", () => {
  it("removes active markup, navigation, external resources, and event handlers", () => {
    const input = `<meta http-equiv="refresh" content="0;url=https://evil.test"><base href="https://evil.test"><script>danger()</script><iframe src="https://evil.test"></iframe><form action="https://evil.test"><input></form><template><img onerror="danger()"></template><a href="javascript:danger()" ping="https://evil.test" target="_top" onclick="danger()">About</a><img src="https://evil.test/x" srcset="https://evil.test/y 2x" onerror="danger()"><svg><use href="https://evil.test/x"></use></svg><div style="background:url(https://evil.test)">Hello</div>`;
    const result = safePreviewHtml(input);
    expect(result).not.toMatch(/evil\.test|javascript:|onerror|onclick|srcset|<script|<iframe|<form|<template|<meta|<base/);
    expect(result).toContain("About");
    expect(result).toContain("Hello");
  });

  it("preserves semantic layout, inline styling, and safe embedded raster images", () => {
    const result = safePreviewHtml('<main><h1>Welcome</h1><p style="color: black">Details</p><img alt="Photo" src="data:image/png;base64,AAAA"></main>');
    expect(result).toContain('<h1>Welcome</h1>');
    expect(result).toContain('style="color: black"');
    expect(result).toContain('src="data:image/png;base64,AAAA"');
  });

  it("puts restrictive policy before generated content and prevents CSS breakout", () => {
    const result = buildPreviewDocument('<h1>Site</h1>', '</style><script>danger()</script>');
    expect(result.indexOf("Content-Security-Policy")).toBeLessThan(result.indexOf("<h1>"));
    const parsed = new DOMParser().parseFromString(result, "text/html");
    expect(parsed.querySelector("script")).toBeNull();
    expect(parsed.querySelector('meta[http-equiv="Content-Security-Policy"]')?.getAttribute("content")).toContain("default-src 'none'");
  });
});

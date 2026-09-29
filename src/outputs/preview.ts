// Defense in depth: this document also runs in an iframe with an empty sandbox.
export const previewCsp = "default-src 'none'; style-src 'unsafe-inline'; img-src data:; font-src data:; form-action 'none'; base-uri 'none';";

export function safePreviewHtml(html: string): string {
  const template = document.createElement("template");
  template.innerHTML = html;
  template.content.querySelectorAll("script, base, iframe, object, embed, link, meta, form, template").forEach((node) => node.remove());
  template.content.querySelectorAll("*").forEach((node) => {
    for (const attribute of [...node.attributes]) {
      const name = attribute.name.toLowerCase();
      if (name.startsWith("on") || ["href", "xlink:href", "srcset", "action", "formaction", "target", "download", "ping"].includes(name)) node.removeAttribute(attribute.name);
      if (name === "src" && !/^data:image\/(?:png|jpeg|gif|webp|avif);/i.test(attribute.value)) node.removeAttribute(attribute.name);
      if (name === "style" && /url\s*\(|@import|expression\s*\(/i.test(attribute.value)) node.removeAttribute(attribute.name);
    }
    if (node.tagName === "STYLE" && /url\s*\(|@import/i.test(node.textContent ?? "")) node.remove();
  });
  return template.innerHTML;
}

export function buildPreviewDocument(html: string, css: string): string {
  const safeCss = css.replace(/<\/style/gi, "<\\/style");
  return `<!doctype html><html><head><meta http-equiv="Content-Security-Policy" content="${previewCsp}"><style>${safeCss}</style></head><body>${safePreviewHtml(html)}</body></html>`;
}

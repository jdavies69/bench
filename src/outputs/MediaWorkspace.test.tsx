// @vitest-environment jsdom
import { afterEach, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MediaWorkspace } from "./MediaWorkspace";
import type { ArtifactState } from "../native";
afterEach(cleanup);
const image: Extract<ArtifactState, { kind: "image" | "voice" }> = { version: 1, kind: "image", revision: 1, requestCount: 1, content: { mimeType: "image/png", dataBase64: "aGVsbG8=", model: "fixture", generationId: null, prompt: "A landscape", voice: null } };
it("shows local image bytes with fit/actual size and native export", async () => {
  const onExport = vi.fn(); render(<MediaWorkspace state={image} busy={false} onExport={onExport} />);
  expect(screen.getByRole("img").getAttribute("src")).toBe("data:image/png;base64,aGVsbG8=");
  await userEvent.click(screen.getByRole("button", { name: "Actual size" })); expect(screen.getByRole("button", { name: "Fit image" }).getAttribute("aria-pressed")).toBe("true");
  await userEvent.click(screen.getByRole("button", { name: "Export image" })); expect(onExport).toHaveBeenCalledTimes(1);
});
it("shows voice script and user-controlled local playback with no autoplay", () => {
  const { container } = render(<MediaWorkspace state={{ ...image, kind: "voice", content: { ...image.content, mimeType: "audio/mpeg", prompt: "A spoken script" } }} busy={false} onExport={vi.fn()} />);
  expect(screen.getByText("A spoken script")).toBeTruthy(); const audio = container.querySelector("audio")!;
  expect(audio.hasAttribute("autoplay")).toBe(false); expect(audio.getAttribute("preload")).toBe("none"); expect(audio.getAttribute("src")).toContain("data:audio/mpeg;base64,");
});
it("rejects active MIME types and remote URLs without creating a media resource", () => {
  const { container } = render(<MediaWorkspace state={{ ...image, content: { ...image.content, mimeType: "image/svg+xml", dataBase64: "https://example.com/track" } }} busy={false} onExport={vi.fn()} />);
  expect(container.querySelector("img")).toBeNull(); expect(screen.getByRole("alert")).toBeTruthy(); expect(screen.getByRole("button", { name: "Export image" }).hasAttribute("disabled")).toBe(true);
});

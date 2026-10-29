import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

import { providerBrandAssets } from "./providerBrandAssets";

const assetRoot = resolve(dirname(fileURLToPath(import.meta.url)), "../../assets/provider-logos");
const sources = JSON.parse(readFileSync(resolve(assetRoot, "sources.json"), "utf8")) as {
  file: string; source: string; sha256: string;
}[];
const svgTags = new Set(["svg", "title", "g", "path", "defs", "clipPath", "rect", "linearGradient", "stop", "circle", "polygon", "ellipse"]);

describe("bundled provider trademark assets", () => {
  it("records every bundled mark and keeps the third-party license", () => {
    expect(sources.map((source) => source.file.replace(/\.(png|svg)$/, "")).sort())
      .toEqual([...providerBrandAssets.keys()].sort());
    expect(readFileSync(resolve(assetRoot, "LICENSE.lobe-icons.txt"), "utf8"))
      .toContain("Copyright (c) 2023 LobeHub");
  });

  it("ships the complete third-party license through the public asset directory", () => {
    const notice = readFileSync(resolve(assetRoot, "../../../public/provider-logos-NOTICE.txt"), "utf8");
    const license = readFileSync(resolve(assetRoot, "LICENSE.lobe-icons.txt"), "utf8");
    expect(notice).toContain(license.trim());
    expect(notice).toContain("https://github.com/lobehub/lobe-icons");
  });

  it.each(sources)("checks provenance and inert content for $file", ({ file, source, sha256 }) => {
    expect(file).toMatch(/^[a-z]+\.(svg|png)$/);
    expect(source).toMatch(/^https:\/\//);
    const bytes = readFileSync(resolve(assetRoot, file));
    expect(bytes.byteLength).toBeLessThan(128 * 1024);
    expect(createHash("sha256").update(bytes).digest("hex")).toBe(sha256);
    if (file.endsWith(".png")) {
      expect(bytes.subarray(0, 8).toString("hex")).toBe("89504e470d0a1a0a");
      return;
    }
    expect(bytes.subarray(0, 3).toString("hex")).not.toBe("efbbbf");
    const xml = new DOMParser().parseFromString(new TextDecoder("utf-8", { fatal: true }).decode(bytes), "image/svg+xml");
    expect(xml.querySelector("parsererror")).toBeNull();
    expect(xml.documentElement.getAttribute("viewBox")).toBeTruthy();
    for (const element of xml.querySelectorAll("*")) {
      expect(svgTags.has(element.localName)).toBe(true);
      for (const attribute of element.attributes) {
        expect(attribute.name).not.toMatch(/^(on|.*href)/i);
        if (attribute.name !== "xmlns") {
          expect(attribute.value).not.toMatch(/javascript:|https?:|@import|url\(\s*[^#]/i);
        }
      }
    }
  });
});

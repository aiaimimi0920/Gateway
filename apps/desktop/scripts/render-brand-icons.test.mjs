import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { after, before, test } from "node:test";
import { chromium } from "playwright";

const root = resolve(import.meta.dirname, "..");
const svg = readFileSync(resolve(root, "public/favicon.svg"), "utf8");
const ico = readFileSync(resolve(root, "src-tauri/icons/icon.ico"));
const png = readFileSync(resolve(root, "src-tauri/icons/icon.png"));
let browser;
let page;

before(async () => {
  browser = await chromium.launch();
  page = await browser.newPage();
});
after(async () => { await browser?.close(); });

function frames() {
  assert.equal(ico.readUInt16LE(0), 0);
  assert.equal(ico.readUInt16LE(2), 1);
  return Array.from({ length: ico.readUInt16LE(4) }, (_, index) => {
    const at = 6 + index * 16;
    const size = ico[at] || 256;
    assert.equal(ico[at + 1] || 256, size);
    assert.equal(ico.readUInt16LE(at + 6), 32);
    const bytes = ico.readUInt32LE(at + 8);
    const offset = ico.readUInt32LE(at + 12);
    assert.ok(offset >= 6 + ico.readUInt16LE(4) * 16 && offset + bytes <= ico.length);
    return { size, data: ico.subarray(offset, offset + bytes) };
  });
}

async function pixels(data, type = "image/png") {
  return page.evaluate(async ({ base64, type }) => {
    const image = new Image();
    image.src = `data:${type};base64,${base64}`;
    await image.decode();
    const canvas = document.createElement("canvas");
    canvas.width = image.naturalWidth;
    canvas.height = image.naturalHeight;
    const context = canvas.getContext("2d");
    context.drawImage(image, 0, 0);
    return {
      width: canvas.width, height: canvas.height,
      rgba: Array.from(context.getImageData(0, 0, canvas.width, canvas.height).data),
    };
  }, { base64: data.toString("base64"), type });
}

function assertTransparentMark(image, size) {
  assert.equal(image.width, size);
  assert.equal(image.height, size);
  let transparent = 0;
  let green = 0;
  let yellow = 0;
  for (let at = 0; at < image.rgba.length; at += 4) {
    const [r, g, b, alpha] = image.rgba.slice(at, at + 4);
    if (alpha === 0) { transparent++; continue; }
    assert.ok(Math.max(r, g, b) > 120, "icon must not contain a dark backplate");
    if (g > r && g > b) green++;
    if (r > b && g > b && r > 100) yellow++;
  }
  assert.ok(transparent > size * size * 0.5, "background must be transparent");
  assert.ok(green > size && yellow > size, "both Gateway brand strokes must remain");
  for (const pixel of [0, size - 1, size * (size - 1), size * size - 1]) {
    assert.equal(image.rgba[pixel * 4 + 3], 0, "corner must be transparent");
  }
}

test("Windows default icon starts with the largest frame, not a stretched 16px frame", () => {
  // tauri-codegen 2.6.3 decodes entries()[0], not the largest available ICO frame.
  const sizes = frames().map(({ size }) => size);
  assert.equal(sizes[0], 256);
  assert.deepEqual(sizes, [256, 128, 96, 64, 48, 40, 32, 24, 20, 16]);
});

test("every Windows DPI frame retains transparent background and brand strokes", async () => {
  for (const frame of frames()) assertTransparentMark(await pixels(frame.data), frame.size);
});

test("512px PNG is transparent and matches the SVG source", async () => {
  const actual = await pixels(png);
  assertTransparentMark(actual, 512);
  await page.setViewportSize({ width: 512, height: 512 });
  await page.setContent(`<!doctype html><style>
    html,body{margin:0;padding:0;background:transparent}
    #host{width:512px;height:512px;overflow:hidden}
    #host svg{display:block;width:512px;height:512px}
  </style><div id="host">${svg}</div>`);
  // Match the generator's DOM rasterisation, not the different SVG-as-image path.
  const expected = await page.locator("#host").screenshot({ omitBackground: true });
  const digest = (bytes) => createHash("sha256").update(bytes).digest("hex");
  assert.equal(digest(png), digest(expected), "regenerate native icons after changing favicon.svg");
});

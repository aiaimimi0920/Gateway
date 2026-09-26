// Rasterises the Gateway brand mark (brackets + signal wave) into the Tauri
// bundle icons. Run from apps/desktop: node scripts/render-brand-icons.mjs
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { chromium } from "playwright";

const root = resolve(import.meta.dirname, "..");
const svgPath = resolve(root, "public/favicon.svg");
const pngPath = resolve(root, "src-tauri/icons/icon.png");
const icoPath = resolve(root, "src-tauri/icons/icon.ico");

// The favicon is the single source of truth for the mark's geometry.
const svg = readFileSync(svgPath, "utf8");

const icoSizes = [16, 24, 32, 48, 64, 128, 256];
const pngSize = 512;

const browser = await chromium.launch();
const page = await browser.newPage();

async function render(size) {
  // Scale the 24-unit artwork up by clamping the SVG element to `size` and
  // letting the browser rasterise the vector at that device resolution.
  const html = `<!doctype html><html><head><style>
    html,body{margin:0;padding:0;background:transparent}
    #host{width:${size}px;height:${size}px;overflow:hidden}
    #host svg{display:block;width:${size}px;height:${size}px}
  </style></head><body><div id="host">${svg}</div></body></html>`;
  await page.setViewportSize({ width: size, height: size });
  await page.setContent(html, { waitUntil: "load" });
  return page.locator("#host").screenshot({ omitBackground: true });
}

function buildIco(entries) {
  const header = Buffer.alloc(6);
  header.writeUInt16LE(0, 0); // reserved
  header.writeUInt16LE(1, 2); // type: icon
  header.writeUInt16LE(entries.length, 4);

  const directory = Buffer.alloc(16 * entries.length);
  let offset = header.length + directory.length;
  entries.forEach((entry, index) => {
    const at = index * 16;
    // 256px is encoded as 0 in the ICO directory.
    directory[at] = entry.size >= 256 ? 0 : entry.size;
    directory[at + 1] = entry.size >= 256 ? 0 : entry.size;
    directory[at + 2] = 0; // palette colours
    directory[at + 3] = 0; // reserved
    directory.writeUInt16LE(1, at + 4); // colour planes
    directory.writeUInt16LE(32, at + 6); // bits per pixel
    directory.writeUInt32LE(entry.data.length, at + 8);
    directory.writeUInt32LE(offset, at + 12);
    offset += entry.data.length;
  });

  return Buffer.concat([header, directory, ...entries.map((entry) => entry.data)]);
}

const icoEntries = [];
for (const size of icoSizes) {
  icoEntries.push({ size, data: await render(size) });
}
const large = await render(pngSize);

await browser.close();

mkdirSync(dirname(pngPath), { recursive: true });
writeFileSync(pngPath, large);
writeFileSync(icoPath, buildIco(icoEntries));

console.log(`icon.png ${pngSize}x${pngSize} ${large.length} bytes`);
console.log(
  `icon.ico ${icoEntries.map((entry) => entry.size).join("/")} -> ${
    buildIco(icoEntries).length
  } bytes`,
);

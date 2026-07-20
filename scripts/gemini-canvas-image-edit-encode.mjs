import { chromium } from "playwright-core";
import { createHash } from "node:crypto";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";

const [inputPath, outputPath, targetBytesArg] = process.argv.slice(2);

if (!inputPath || !outputPath) {
  console.error(JSON.stringify({ ok: false, error: "usage: node gemini-canvas-image-edit-encode.mjs <input> <output> [targetBytes]" }));
  process.exit(1);
}

const targetBytes = Number.parseInt(targetBytesArg ?? "127467", 10);
const executablePath = [
  process.env.GEMINI_CANVAS_BROWSER_EXECUTABLE_PATH,
  "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe",
  "C:\\Program Files\\Microsoft\\Edge\\Application\\msedge.exe",
  "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
].find((candidate) => candidate && existsSync(candidate));

if (!executablePath) {
  console.error(JSON.stringify({ ok: false, error: "no chromium-compatible browser found" }));
  process.exit(1);
}

const inputBytes = readFileSync(inputPath);
const inputExt = path.extname(inputPath).toLowerCase();
const inferredMime =
  inputExt === ".jpg" || inputExt === ".jpeg"
    ? "image/jpeg"
    : inputExt === ".webp"
      ? "image/webp"
      : inputExt === ".gif"
        ? "image/gif"
        : "image/png";
const inputDataUrl = `data:${inferredMime};base64,${inputBytes.toString("base64")}`;

const browser = await chromium.launch({
  executablePath,
  headless: true,
  args: ["--disable-dev-shm-usage", "--no-first-run", "--no-default-browser-check"],
});

try {
  const page = await browser.newPage();
  await page.goto("about:blank");
  const result = await page.evaluate(
    async ({ inputDataUrl, targetBytes }) => {
      const img = new Image();
      img.src = inputDataUrl;
      await new Promise((resolve, reject) => {
        img.onload = resolve;
        img.onerror = reject;
      });

      const sizes = [
        1152, 1170, 1180, 1190, 1197, 1200, 1210,
        1214, 1215, 1216, 1217, 1218, 1219, 1220, 1221, 1222, 1223, 1224,
        1240, 1260, 1280, 1320, 1400,
      ];
      const qualities = [0.84, 0.85, 0.86, 0.9, 0.92];
      const aspect = img.width / img.height;
      let best = null;

      for (const longEdge of sizes) {
        let width;
        let height;
        if (aspect >= 1) {
          width = longEdge;
          height = Math.max(1, Math.round(longEdge / aspect));
        } else {
          height = longEdge;
          width = Math.max(1, Math.round(longEdge * aspect));
        }
        const canvas = document.createElement("canvas");
        canvas.width = width;
        canvas.height = height;
        const ctx = canvas.getContext("2d");
        ctx.drawImage(img, 0, 0, width, height);
        for (const quality of qualities) {
          const blob = await new Promise((resolve) => canvas.toBlob(resolve, "image/jpeg", quality));
          if (!blob) continue;
          const dataUrl = await new Promise((resolve, reject) => {
            const reader = new FileReader();
            reader.onload = () => resolve(reader.result);
            reader.onerror = reject;
            reader.readAsDataURL(blob);
          });
          const base64 = String(dataUrl).split(",", 2)[1] ?? "";
          const candidate = {
            width,
            height,
            longEdge,
            quality,
            byteLength: blob.size,
            diff: Math.abs(blob.size - targetBytes),
            base64,
          };
          if (
            !best ||
            candidate.diff < best.diff ||
            (candidate.diff === best.diff && candidate.byteLength > best.byteLength)
          ) {
            best = candidate;
          }
        }
      }

      return {
        sourceWidth: img.width,
        sourceHeight: img.height,
        targetBytes,
        best,
      };
    },
    { inputDataUrl, targetBytes },
  );

  if (!result?.best?.base64) {
    throw new Error("browser encoder did not produce a JPEG candidate");
  }

  const outputBytes = Buffer.from(result.best.base64, "base64");
  writeFileSync(outputPath, outputBytes);
  const sha256 = createHash("sha256").update(outputBytes).digest("hex");
  console.log(
    JSON.stringify({
      ok: true,
      executablePath,
      inputPath,
      outputPath,
      outputSha256: sha256,
      sourceWidth: result.sourceWidth,
      sourceHeight: result.sourceHeight,
      targetBytes: result.targetBytes,
      selected: {
        width: result.best.width,
        height: result.best.height,
        longEdge: result.best.longEdge,
        quality: result.best.quality,
        byteLength: result.best.byteLength,
        diff: result.best.diff,
      },
    }),
  );
} finally {
  await browser.close();
}

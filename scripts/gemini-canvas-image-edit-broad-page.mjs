import { Buffer } from "node:buffer";
import { createHash } from "node:crypto";
import { writeFileSync } from "node:fs";
import path from "node:path";

export async function collectImageEditBroadPageState(page) {
  return await page.evaluate(() => {
    const imageNodes = Array.from(document.querySelectorAll("img"))
      .map((img, i) => ({
        i,
        alt: img.getAttribute("alt"),
        src: img.getAttribute("src"),
        width: img.naturalWidth,
        height: img.naturalHeight,
      }))
      .filter((item) => item.src && !item.src.startsWith("data:image/gif"));

    const anchorNodes = Array.from(document.querySelectorAll("a"))
      .map((node, i) => ({
        i,
        text: (node.textContent || "").trim(),
        href: node.getAttribute("href"),
      }))
      .filter((node) => node.href || node.text);

    const cssBgNodes = Array.from(document.querySelectorAll("*"))
      .map((node, i) => {
        const style = getComputedStyle(node);
        const backgroundImage = style.backgroundImage || "";
        if (backgroundImage === "none") {
          return null;
        }
        if (!/googleusercontent|gg-dl|rd-gg-dl|data:image|blob:/i.test(backgroundImage)) {
          return null;
        }
        return {
          i,
          tag: node.tagName,
          className: String(node.className || "").slice(0, 200),
          backgroundImage: backgroundImage.slice(0, 800),
          width: node.clientWidth,
          height: node.clientHeight,
          text: (node.textContent || "").trim().slice(0, 200),
        };
      })
      .filter(Boolean);

    const canvasNodes = Array.from(document.querySelectorAll("canvas"))
      .map((node, i) => ({
        i,
        width: node.width,
        height: node.height,
        clientWidth: node.clientWidth,
        clientHeight: node.clientHeight,
      }))
      .filter((node) => (node.width ?? 0) >= 256 || (node.height ?? 0) >= 256);

    const buttonNodes = Array.from(document.querySelectorAll("button"))
      .map((node, i) => ({
        i,
        text: (node.textContent || "").trim(),
        aria: node.getAttribute("aria-label"),
      }))
      .filter((node) => node.text || node.aria);

    return {
      imageNodes,
      anchorNodes,
      cssBgNodes,
      canvasNodes,
      buttonNodes,
      pageState: {
        url: location.href,
        title: document.title,
        bodyText: document.body.innerText.slice(0, 12000),
      },
    };
  });
}

export async function exportImageEditBroadPageBlobs(page, outDir) {
  const pageBlobCaptures = await page.evaluate(async () => {
    const nodes = Array.from(document.querySelectorAll("img"))
      .map((img, i) => ({
        i,
        alt: img.getAttribute("alt") || "",
        src: img.getAttribute("src") || "",
      }))
      .filter((node) => node.src.startsWith("blob:"));

    const toBase64 = (bytes) => {
      let binary = "";
      const chunkSize = 0x8000;
      for (let offset = 0; offset < bytes.length; offset += chunkSize) {
        const slice = bytes.subarray(offset, offset + chunkSize);
        binary += String.fromCharCode(...slice);
      }
      return btoa(binary);
    };

    const captures = [];
    for (const node of nodes) {
      try {
        const response = await fetch(node.src);
        const mimeType = response.headers.get("content-type") || "application/octet-stream";
        const bytes = new Uint8Array(await response.arrayBuffer());
        captures.push({
          ...node,
          mimeType,
          byteLength: bytes.length,
          dataBase64: toBase64(bytes),
        });
      } catch (error) {
        captures.push({
          ...node,
          error: String(error),
        });
      }
    }
    return captures;
  });

  const extensionForMimeType = (mimeType) => {
    const normalized = String(mimeType || "").toLowerCase();
    if (normalized.includes("jpeg") || normalized.includes("jpg")) return "jpg";
    if (normalized.includes("png")) return "png";
    if (normalized.includes("webp")) return "webp";
    if (normalized.includes("gif")) return "gif";
    return "bin";
  };

  const slugifyLabel = (value) =>
    String(value || "")
      .toLowerCase()
      .replace(/ai\s*生成/g, "ai-generated")
      .replace(/所上传图片的预览图/g, "uploaded-preview")
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-+|-+$/g, "")
      .slice(0, 48) || "blob";

  return pageBlobCaptures.map((capture) => {
    if (!capture?.dataBase64) {
      return {
        i: capture?.i ?? null,
        alt: capture?.alt ?? "",
        src: capture?.src ?? "",
        error: capture?.error ?? "missing blob data",
      };
    }
    const ext = extensionForMimeType(capture.mimeType);
    const label = slugifyLabel(capture.alt);
    const fileName = `page-blob-${label}-${capture.i}.${ext}`;
    const outputPath = path.join(outDir, fileName);
    const buffer = Buffer.from(capture.dataBase64, "base64");
    writeFileSync(outputPath, buffer);
    return {
      i: capture.i,
      alt: capture.alt,
      src: capture.src,
      mimeType: capture.mimeType,
      byteLength: capture.byteLength,
      sha256: createHash("sha256").update(buffer).digest("hex"),
      fileName,
      outputPath,
    };
  });
}

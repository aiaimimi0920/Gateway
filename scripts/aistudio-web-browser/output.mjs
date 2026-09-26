import { mkdtemp, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { stripUtf8Bom } from "./payload.mjs";
import { normalizeString } from "./settings.mjs";

const MAX_INLINE_TEXT_BODY_CHARS = 24_000;

export async function readStdin() {
  const chunks = [];
  for await (const chunk of process.stdin) {
    chunks.push(Buffer.from(chunk));
  }
  return stripUtf8Bom(Buffer.concat(chunks).toString("utf8"));
}

export async function printJsonAndExit(payload, exitCode = 0, resultFilePath = null) {
  const serialized = `${JSON.stringify(payload)}\n`;
  if (normalizeString(resultFilePath)) {
    await writeFile(resultFilePath, serialized, "utf8");
  } else {
    await new Promise((resolve, reject) => {
      process.stdout.write(serialized, "utf8", (error) => {
        if (error) {
          reject(error);
        } else {
          resolve();
        }
      });
    });
  }
  process.exitCode = exitCode;
}

export async function maybeExternalizeLargeTextBody(result) {
  if (
    !result ||
    typeof result !== "object" ||
    typeof result.bodyText !== "string" ||
    result.bodyText.length <= MAX_INLINE_TEXT_BODY_CHARS
  ) {
    return result;
  }
  const dir = await mkdtemp(path.join(tmpdir(), "aistudio-browser-worker-"));
  const extension = (result.contentType || "").includes("json") ? ".json" : ".txt";
  const filePath = path.join(dir, `response${extension}`);
  await writeFile(filePath, result.bodyText, "utf8");
  return {
    ...result,
    bodyText: null,
    bodyFilePath: filePath,
  };
}

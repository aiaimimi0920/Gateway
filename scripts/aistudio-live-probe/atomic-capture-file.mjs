import fs from "node:fs/promises";
import { randomUUID } from "node:crypto";
import path from "node:path";

export async function writeAtomicCapture(filePath, content, encoding = "utf8") {
  const temporary = path.join(path.dirname(filePath), `.${path.basename(filePath)}.${randomUUID()}.tmp`);
  let handle = null;
  let created = false;
  try {
    handle = await fs.open(temporary, "wx", 0o600);
    created = true;
    await handle.writeFile(content, encoding);
    await handle.sync();
    await handle.close();
    handle = null;
    // Same-directory replacement exposes a complete file to readers. Do not
    // delete the destination as a fallback when replacement is unavailable.
    await fs.rename(temporary, filePath);
    created = false;
  } finally {
    await handle?.close().catch(() => {});
    if (created) await fs.unlink(temporary).catch(() => {});
  }
}

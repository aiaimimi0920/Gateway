import { createReadStream } from "node:fs";
import { stripUtf8Bom } from "./input-text.mjs";
import { withObjectStorageRequest, readRuntimeStateBody } from "./object-storage-request.mjs";

export async function readRuntimeStateFile(filePath) {
  let bytes;
  try {
    bytes = await withObjectStorageRequest((signal) => {
      signal.throwIfAborted();
      return readRuntimeStateBody(createReadStream(filePath, { signal, highWaterMark: 65536 }), signal);
    });
  } catch (error) {
    if (error?.code === "aistudio_runtime_state_too_large") throw error;
    const timedOut = error?.code === "aistudio_object_storage_timeout";
    throw Object.assign(new Error(timedOut
      ? "AI Studio runtime state file read exceeded 30 seconds."
      : "AI Studio runtime state file could not be read."), {
      code: timedOut ? "aistudio_runtime_state_read_timeout" : "aistudio_runtime_state_read_failed",
      status: timedOut ? 504 : 500,
    });
  }
  try {
    return JSON.parse(stripUtf8Bom(bytes.toString("utf8")));
  } catch {
    throw Object.assign(new Error("AI Studio runtime state file must contain valid JSON."), {
      code: "aistudio_runtime_state_invalid_json", status: 400,
    });
  }
}

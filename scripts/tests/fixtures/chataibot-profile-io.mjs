export * from "node:fs/promises";
import { copyFile as realCopyFile } from "node:fs/promises";

export async function copyFile(...args) {
  if (process.env.CHATAIBOT_TEST_FAILURE === "copy" && /[\\/]Preferences$/.test(args[0])) {
    throw Object.assign(new Error("fixture copy failed"), { code: "EIO" });
  }
  return realCopyFile(...args);
}

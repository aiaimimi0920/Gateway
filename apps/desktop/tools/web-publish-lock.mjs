import { randomUUID } from "node:crypto";
import { mkdir, readFile, readdir, rmdir, stat, writeFile } from "node:fs/promises";
import path from "node:path";
import { removePathWithRetry, sleep } from "./web-publish-cleanup.mjs";

const PUBLISH_LOCK_TIMEOUT_MS = 60_000;
const PUBLISH_LOCK_RETRY_MS = 50;
const PUBLISH_LOCK_INCOMPLETE_OWNER_GRACE_MS = 10_000;
const PUBLISH_LOCK_OWNER_FILE = "owner.json";
const PUBLISH_LOCK_CONTENTION_CODES = new Set(["EEXIST", "EPERM"]);

function processIsAlive(pid) {
  if (!Number.isSafeInteger(pid) || pid <= 0) {
    return undefined;
  }
  try {
    process.kill(pid, 0);
    return true;
  } catch (error) {
    if (error?.code === "ESRCH") {
      return false;
    }
    if (error?.code === "EPERM") {
      return true;
    }
    return undefined;
  }
}

async function readPublishLockOwnerSnapshot(ownerFile) {
  try {
    const [ownerText, ownerStat] = await Promise.all([
      readFile(ownerFile, "utf8"),
      stat(ownerFile),
    ]);
    return {
      ownerText,
      modifiedAtMs: ownerStat.mtimeMs,
      size: ownerStat.size,
    };
  } catch (error) {
    if (error?.code === "ENOENT") {
      return undefined;
    }
    throw error;
  }
}

async function readPublishLockDirSnapshot(lockDir) {
  try {
    const [entries, lockStat] = await Promise.all([
      readdir(lockDir),
      stat(lockDir),
    ]);
    return {
      entryCount: entries.length,
      isDirectory: lockStat.isDirectory(),
      modifiedAtMs: lockStat.mtimeMs,
    };
  } catch (error) {
    if (error?.code === "ENOENT") {
      return undefined;
    }
    throw error;
  }
}

// Reclaims a lock directory that never received an owner file, left behind by a
// publisher killed between creating the directory and writing the file. Without
// this such a directory is unrecoverable and every later publish times out.
//
// A live publisher stays safe without an owner to read: the directory has to still
// be empty, it has to have sat untouched longer than the gap between creating it
// and writing the owner file could ever be, and the removal is non-recursive so it
// fails the moment a racing publisher has written anything into it.
async function recoverOwnerlessPublishLock(lockDir) {
  const snapshot = await readPublishLockDirSnapshot(lockDir);
  if (!snapshot) {
    // Already cleared, so the caller only has to retry the create.
    return true;
  }
  if (!snapshot.isDirectory || snapshot.entryCount > 0) {
    return false;
  }
  if (
    Date.now() - snapshot.modifiedAtMs <
    PUBLISH_LOCK_INCOMPLETE_OWNER_GRACE_MS
  ) {
    return false;
  }
  try {
    await rmdir(lockDir);
    return true;
  } catch (error) {
    if (error?.code === "ENOENT") {
      return true;
    }
    // ENOTEMPTY means a racing publisher filled the directory after the check
    // above, so it owns the lock and nothing of its is removed here.
    return false;
  }
}

async function recoverAbandonedPublishLock(lockDir, ownerFile) {
  const ownerSnapshot = await readPublishLockOwnerSnapshot(ownerFile);
  if (!ownerSnapshot) {
    return recoverOwnerlessPublishLock(lockDir);
  }

  let owner;
  try {
    owner = JSON.parse(ownerSnapshot.ownerText);
  } catch {
    if (
      Date.now() - ownerSnapshot.modifiedAtMs <
      PUBLISH_LOCK_INCOMPLETE_OWNER_GRACE_MS
    ) {
      return false;
    }

    const currentOwnerSnapshot = await readPublishLockOwnerSnapshot(ownerFile);
    if (
      !currentOwnerSnapshot ||
      currentOwnerSnapshot.ownerText !== ownerSnapshot.ownerText ||
      currentOwnerSnapshot.modifiedAtMs !== ownerSnapshot.modifiedAtMs ||
      currentOwnerSnapshot.size !== ownerSnapshot.size
    ) {
      return false;
    }

    await removePathWithRetry(lockDir, { recursive: true, force: true });
    return true;
  }
  if (
    typeof owner?.ownerToken !== "string" ||
    owner.ownerToken.length === 0 ||
    processIsAlive(owner.pid) !== false
  ) {
    return false;
  }

  const currentOwnerSnapshot = await readPublishLockOwnerSnapshot(ownerFile);
  if (
    !currentOwnerSnapshot ||
    currentOwnerSnapshot.ownerText !== ownerSnapshot.ownerText
  ) {
    return false;
  }

  await removePathWithRetry(lockDir, { recursive: true, force: true });
  return true;
}

export async function acquirePublishLock(lockDir) {
  await mkdir(path.dirname(lockDir), { recursive: true });
  const deadline = Date.now() + PUBLISH_LOCK_TIMEOUT_MS;
  const ownerToken = randomUUID();
  const ownerFile = path.join(lockDir, PUBLISH_LOCK_OWNER_FILE);

  while (true) {
    let lockDirectoryCreated = false;
    try {
      await mkdir(lockDir);
      lockDirectoryCreated = true;
      try {
        await writeFile(
          ownerFile,
          `${JSON.stringify({
            ownerToken,
            pid: process.pid,
            acquiredAt: new Date().toISOString(),
          })}\n`,
          { encoding: "utf8", flag: "wx" },
        );
      } catch (error) {
        await removePathWithRetry(lockDir, { recursive: true, force: true });
        throw error;
      }

      return async () => {
        const owner = JSON.parse(await readFile(ownerFile, "utf8"));
        if (owner?.ownerToken !== ownerToken) {
          throw new Error(
            `web publish lock owner changed before release: ${lockDir}`,
          );
        }
        await removePathWithRetry(lockDir, { recursive: true, force: true });
      };
    } catch (error) {
      if (lockDirectoryCreated) {
        throw error;
      }
      if (!PUBLISH_LOCK_CONTENTION_CODES.has(error?.code)) {
        throw error;
      }
      if (await recoverAbandonedPublishLock(lockDir, ownerFile)) {
        continue;
      }
      if (Date.now() >= deadline) {
        throw new Error(`timed out waiting for web publish lock: ${lockDir}`);
      }
      await sleep(PUBLISH_LOCK_RETRY_MS);
    }
  }
}

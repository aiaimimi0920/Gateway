import { createHash, randomUUID } from "node:crypto";
import {
  copyFile,
  cp,
  mkdir,
  readFile,
  readdir,
  rename,
  writeFile,
} from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { acquirePublishLock } from "./web-publish-lock.mjs";
import { removePathWithRetry } from "./web-publish-cleanup.mjs";

const READY_MARKER_SCHEMA_VERSION = 1;
async function assertFile(filePath, description) {
  try {
    const contents = await readFile(filePath);
    return contents;
  } catch (error) {
    if (error?.code === "ENOENT") {
      throw new Error(`${description} is missing: ${filePath}`);
    }
    throw error;
  }
}

async function replaceTemporaryFile(temporaryPath, destinationPath) {
  try {
    await rename(temporaryPath, destinationPath);
  } catch (error) {
    if (!new Set(["EACCES", "EEXIST", "EPERM"]).has(error?.code)) {
      throw error;
    }
    await removePathWithRetry(destinationPath, { force: true });
    await rename(temporaryPath, destinationPath);
  }
}

async function replaceFile(sourcePath, destinationPath) {
  const temporaryPath = `${destinationPath}.next-${process.pid}-${randomUUID()}`;
  await mkdir(path.dirname(destinationPath), { recursive: true });
  try {
    await copyFile(sourcePath, temporaryPath);
    await replaceTemporaryFile(temporaryPath, destinationPath);
  } finally {
    await removePathWithRetry(temporaryPath, { force: true });
  }
}

async function replaceFileContents(destinationPath, contents) {
  const temporaryPath = `${destinationPath}.next-${process.pid}-${randomUUID()}`;
  await mkdir(path.dirname(destinationPath), { recursive: true });
  try {
    await writeFile(temporaryPath, contents);
    await replaceTemporaryFile(temporaryPath, destinationPath);
  } finally {
    await removePathWithRetry(temporaryPath, { force: true });
  }
}

async function replaceReadyMarker(readyFile, content) {
  await replaceFileContents(readyFile, content);
}

async function readOptionalFile(filePath) {
  try {
    return { exists: true, contents: await readFile(filePath) };
  } catch (error) {
    if (error?.code === "ENOENT") {
      return { exists: false, contents: undefined };
    }
    throw error;
  }
}

async function restoreFile(filePath, previousFile) {
  if (previousFile.exists) {
    await replaceFileContents(filePath, previousFile.contents);
  } else {
    await removePathWithRetry(filePath, { force: true });
  }
}

async function collectFiles(rootDir, currentDir = rootDir) {
  const files = [];
  for (const entry of await readdir(currentDir, { withFileTypes: true })) {
    const entryPath = path.join(currentDir, entry.name);
    if (entry.isDirectory()) {
      files.push(...(await collectFiles(rootDir, entryPath)));
    } else if (entry.isFile()) {
      files.push(path.relative(rootDir, entryPath));
    }
  }
  return files.sort();
}

function portableRelativePath(relativePath) {
  return relativePath.split(path.sep).join("/");
}

function sha256(contents) {
  return createHash("sha256").update(contents).digest("hex");
}

async function collectFileDigests(rootDir, relativePaths) {
  const digests = {};
  for (const relativePath of relativePaths) {
    digests[portableRelativePath(relativePath)] = sha256(
      await assertFile(
        path.join(rootDir, relativePath),
        "web console publish snapshot file",
      ),
    );
  }
  return digests;
}

async function validatePublishedFiles(liveDir, expectedFiles) {
  for (const [relativePath, expectedDigest] of Object.entries(expectedFiles)) {
    const actualDigest = sha256(
      await assertFile(
        path.join(liveDir, ...relativePath.split("/")),
        "published web console file",
      ),
    );
    if (actualDigest !== expectedDigest) {
      throw new Error(
        `published web console file changed before ready signalling: ${relativePath}`,
      );
    }
  }
}

async function removeEmptyDirectories(rootDir, currentDir = rootDir) {
  for (const entry of await readdir(currentDir, { withFileTypes: true })) {
    if (entry.isDirectory()) {
      await removeEmptyDirectories(rootDir, path.join(currentDir, entry.name));
    }
  }
  if (currentDir !== rootDir && (await readdir(currentDir)).length === 0) {
    await removePathWithRetry(currentDir, { recursive: true });
  }
}

async function pruneLiveFiles(
  liveDir,
  publishedFiles,
  beforeRemove = async () => {},
) {
  const published = new Set(publishedFiles);
  for (const relativePath of await collectFiles(liveDir)) {
    if (!published.has(relativePath)) {
      await beforeRemove(relativePath);
      await removePathWithRetry(path.join(liveDir, relativePath), {
        force: true,
      });
    }
  }
  await removeEmptyDirectories(liveDir);
}

async function validateCommittedMarker(
  readyFile,
  expectedDigest,
  expectedFiles,
) {
  const marker = JSON.parse(await readFile(readyFile, "utf8"));
  if (marker?.schemaVersion !== READY_MARKER_SCHEMA_VERSION) {
    throw new Error(
      `published web console marker has an unsupported schema version: ${readyFile}`,
    );
  }
  if (marker?.indexSha256 !== expectedDigest) {
    throw new Error(
      `published web console marker does not match index digest: ${readyFile}`,
    );
  }
  if (
    !marker.files ||
    typeof marker.files !== "object" ||
    Array.isArray(marker.files)
  ) {
    throw new Error(
      `published web console marker has no file digest manifest: ${readyFile}`,
    );
  }
  const expectedEntries = Object.entries(expectedFiles);
  const markerEntries = Object.entries(marker.files);
  if (
    markerEntries.length !== expectedEntries.length ||
    expectedEntries.some(
      ([relativePath, digest]) => marker.files[relativePath] !== digest,
    )
  ) {
    throw new Error(
      `published web console marker does not match file digest manifest: ${readyFile}`,
    );
  }
}

function combineErrors(message, errors) {
  return errors.length === 1 ? errors[0] : new AggregateError(errors, message);
}

export async function publishWebDist(
  { stagingDir, liveDir, readyFile, pruneLive = false },
  operations = {},
) {
  const resolvedStagingDir = path.resolve(stagingDir);
  const resolvedLiveDir = path.resolve(liveDir);
  const resolvedReadyFile = path.resolve(readyFile);
  const publishLock = path.join(
    path.dirname(resolvedReadyFile),
    ".gateway-web-publish.lock",
  );
  const stagingIndex = path.join(resolvedStagingDir, "index.html");
  await assertFile(stagingIndex, "staged web console index");

  const snapshotDir = `${resolvedLiveDir}.publish-${process.pid}-${randomUUID()}`;
  const commitReadyMarker =
    operations.replaceReadyMarker ?? replaceReadyMarker;
  let releasePublishLock;
  let previousIndex;
  let previousMarker;
  const previousLiveFiles = new Map();
  let liveMutationStarted = false;
  let indexMutationStarted = false;
  let readyMarkerInvalidated = false;
  let commitComplete = false;
  let result;
  let operationError;

  try {
    await cp(resolvedStagingDir, snapshotDir, { recursive: true });

    const snapshotIndex = path.join(snapshotDir, "index.html");
    const snapshotIndexContents = await assertFile(
      snapshotIndex,
      "web console publish snapshot index",
    );
    const snapshotFiles = await collectFiles(snapshotDir);
    const snapshotFileDigests = await collectFileDigests(
      snapshotDir,
      snapshotFiles,
    );
    releasePublishLock = await acquirePublishLock(publishLock);
    await mkdir(resolvedLiveDir, { recursive: true });
    previousIndex = await readOptionalFile(
      path.join(resolvedLiveDir, "index.html"),
    );
    previousMarker = await readOptionalFile(resolvedReadyFile);
    await removePathWithRetry(resolvedReadyFile, { force: true });
    readyMarkerInvalidated = true;

    for (const relativePath of snapshotFiles) {
      if (relativePath === "index.html") {
        continue;
      }
      const destinationPath = path.join(resolvedLiveDir, relativePath);
      previousLiveFiles.set(
        relativePath,
        await readOptionalFile(destinationPath),
      );
      liveMutationStarted = true;
      await replaceFile(
        path.join(snapshotDir, relativePath),
        destinationPath,
      );
    }

    indexMutationStarted = true;
    await replaceFile(snapshotIndex, path.join(resolvedLiveDir, "index.html"));

    const indexDigest = sha256(snapshotIndexContents);
    const liveIndexDigest = sha256(
      await readFile(path.join(resolvedLiveDir, "index.html")),
    );
    if (liveIndexDigest !== indexDigest) {
      throw new Error("published web console index changed before ready signalling");
    }
    if (pruneLive) {
      await pruneLiveFiles(
        resolvedLiveDir,
        snapshotFiles,
        async (relativePath) => {
          const destinationPath = path.join(resolvedLiveDir, relativePath);
          if (!previousLiveFiles.has(relativePath)) {
            previousLiveFiles.set(
              relativePath,
              await readOptionalFile(destinationPath),
            );
          }
          liveMutationStarted = true;
        },
      );
    }

    await validatePublishedFiles(resolvedLiveDir, snapshotFileDigests);
    const marker = `${JSON.stringify({
      schemaVersion: READY_MARKER_SCHEMA_VERSION,
      indexSha256: indexDigest,
      files: snapshotFileDigests,
      publishedAt: new Date().toISOString(),
    })}\n`;

    await commitReadyMarker(resolvedReadyFile, marker);
    await validateCommittedMarker(
      resolvedReadyFile,
      indexDigest,
      snapshotFileDigests,
    );
    commitComplete = true;

    result = {
      indexDigest,
      liveDir: resolvedLiveDir,
      readyFile: resolvedReadyFile,
    };
  } catch (error) {
    operationError = error;
    if (
      releasePublishLock &&
      (readyMarkerInvalidated || liveMutationStarted || indexMutationStarted) &&
      !commitComplete &&
      previousIndex &&
      previousMarker
    ) {
      const rollbackErrors = [];
      for (const [relativePath, previousFile] of [
        ...previousLiveFiles.entries(),
      ].reverse()) {
        try {
          await restoreFile(
            path.join(resolvedLiveDir, relativePath),
            previousFile,
          );
        } catch (rollbackError) {
          rollbackErrors.push(rollbackError);
        }
      }
      try {
        await removeEmptyDirectories(resolvedLiveDir);
      } catch (rollbackError) {
        rollbackErrors.push(rollbackError);
      }
      try {
        await restoreFile(
          path.join(resolvedLiveDir, "index.html"),
          previousIndex,
        );
      } catch (rollbackError) {
        rollbackErrors.push(rollbackError);
      }
      try {
        await restoreFile(resolvedReadyFile, previousMarker);
      } catch (rollbackError) {
        rollbackErrors.push(rollbackError);
      }
      if (rollbackErrors.length > 0) {
        operationError = combineErrors("web publish and rollback failed", [
          operationError,
          ...rollbackErrors,
        ]);
      }
    }
  } finally {
    const cleanupErrors = [];
    if (releasePublishLock) {
      try {
        await releasePublishLock();
      } catch (error) {
        cleanupErrors.push(error);
      }
    }
    try {
      await removePathWithRetry(snapshotDir, {
        recursive: true,
        force: true,
      });
    } catch (error) {
      cleanupErrors.push(error);
    }
    if (cleanupErrors.length > 0) {
      operationError = combineErrors("web publish cleanup failed", [
        ...(operationError ? [operationError] : []),
        ...cleanupErrors,
      ]);
    }
  }

  if (operationError) {
    throw operationError;
  }
  return result;
}

function parseArguments(argv) {
  const values = new Map();
  let pruneLive = false;
  for (let index = 0; index < argv.length; ) {
    const key = argv[index];
    if (key === "--prune") {
      pruneLive = true;
      index += 1;
      continue;
    }
    const value = argv[index + 1];
    if (!key?.startsWith("--") || !value) {
      throw new Error(
        "usage: publish-web-dist.mjs --staging <dir> --live <dir> --ready <file> [--prune]",
      );
    }
    values.set(key.slice(2), value);
    index += 2;
  }
  for (const required of ["staging", "live", "ready"]) {
    if (!values.has(required)) {
      throw new Error(`missing required argument: --${required}`);
    }
  }
  return {
    stagingDir: values.get("staging"),
    liveDir: values.get("live"),
    readyFile: values.get("ready"),
    pruneLive,
  };
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  publishWebDist(parseArguments(process.argv.slice(2))).catch((error) => {
    console.error(error instanceof Error ? error.stack : error);
    process.exitCode = 1;
  });
}

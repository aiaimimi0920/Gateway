export interface PublishWebDistOptions {
  stagingDir: string;
  liveDir: string;
  readyFile: string;
  pruneLive?: boolean;
}

export interface PublishWebDistOperations {
  replaceReadyMarker?: (readyFile: string, content: string) => Promise<void>;
}

export interface PublishWebDistResult {
  indexDigest: string;
  liveDir: string;
  readyFile: string;
}

export function publishWebDist(
  options: PublishWebDistOptions,
  operations?: PublishWebDistOperations,
): Promise<PublishWebDistResult>;

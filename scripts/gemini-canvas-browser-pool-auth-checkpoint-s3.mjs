// Dedicated checkpoint objects use conditional PUT, never overwrite imported JSON.
import { S3Client, GetObjectCommand, PutObjectCommand } from "@aws-sdk/client-s3";
import { readRuntimeStateBody } from "./aistudio-live-probe/object-storage-request.mjs";
import { boundedCheckpointTask, checkpointError } from "./gemini-canvas-browser-pool-auth-checkpoint-codec.mjs";

function etag(value) {
  if (typeof value !== "string" || !/^"[^"\r\n]{1,256}"$/.test(value)) throw checkpointError("etag_required");
  return value;
}

async function withClient(config, operation) {
  const client = new S3Client({ region: config.region, endpoint: config.endpoint,
    forcePathStyle: config.forcePathStyle, maxAttempts: 1, followRegionRedirects: false,
    credentials: { accessKeyId: config.accessKeyId, secretAccessKey: config.secretAccessKey } });
  try {
    return await boundedCheckpointTask(signal => operation(client, signal));
  } catch (error) {
    if (error?.code?.startsWith("gemini_canvas_auth_checkpoint_")) throw error;
    if ([409, 412].includes(error?.$metadata?.httpStatusCode)) throw checkpointError("conflict", 409);
    throw checkpointError("remote_storage_failed");
  } finally {
    client.destroy();
  }
}

const objectKey = (config, binding) => `${config.prefix}/${binding}.checkpoint`;

export async function readS3Checkpoint(config, binding) {
  return withClient(config, async (client, signal) => {
    let response;
    try {
      response = await client.send(new GetObjectCommand({ Bucket: config.bucket, Key: objectKey(config, binding) }), { abortSignal: signal });
    } catch (error) {
      if (error?.$metadata?.httpStatusCode === 404) return { token: null, bytes: null };
      throw error;
    }
    const bytes = await readRuntimeStateBody(response.Body, signal, response.ContentLength);
    return { token: etag(response.ETag), bytes };
  });
}

export async function writeS3Checkpoint(config, binding, token, record) {
  return withClient(config, async (client, signal) => {
    const response = await client.send(new PutObjectCommand({ Bucket: config.bucket, Key: objectKey(config, binding),
      Body: record.bytes, ContentType: "application/octet-stream",
      ...(token === null ? { IfNoneMatch: "*" } : { IfMatch: etag(token) }) }), { abortSignal: signal });
    return etag(response.ETag);
  });
}

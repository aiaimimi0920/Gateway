import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const filename = fileURLToPath(import.meta.url);
const mode = path.basename(filename, '.mjs');
const limit = 2 * 1024 * 1024;
const watchdog = setTimeout(() => process.exit(99), 12000);
fs.writeFileSync(filename + '.pid', String(process.pid));

function write(stream, bytes) {
  return new Promise((resolve, reject) => {
    stream.write(bytes, (error) => error ? reject(error) : resolve());
  });
}

function jsonAtLength(length) {
  const prefix = Buffer.from('{"credentials":[],"prune":[],"message":"');
  const bytes = Buffer.alloc(length, 0x61);
  prefix.copy(bytes);
  bytes.write('"}', length - 2);
  return bytes;
}

async function captureRequest() {
  const chunks = [];
  let size = 0;
  for await (const chunk of process.stdin) {
    size += chunk.length;
    if (size > 8 * limit) throw new Error('fixture request exceeded its limit');
    chunks.push(chunk);
  }
  const bytes = Buffer.concat(chunks);
  JSON.parse(bytes.toString('utf8'));
  fs.writeFileSync(filename + '.request.json', bytes);
}

async function run() {
  if (mode === 'ignore-input') return new Promise(() => {});
  if (mode === 'closed-input') {
    process.stdin.destroy();
    return new Promise(() => {});
  }
  if (mode === 'stdout-duplex') await write(process.stdout, jsonAtLength(limit));
  if (mode === 'stderr-duplex') await write(process.stderr, Buffer.alloc(4 * limit, 0x61));
  await captureRequest();
  if (mode === 'stdout-duplex') return;
  if (mode === 'wait-timeout') return new Promise(() => {});
  if (mode === 'nonzero') {
    process.exitCode = 7;
    return;
  }
  if (mode === 'malformed') return write(process.stdout, Buffer.from('not-json'));
  if (mode === 'exact-limit') return write(process.stdout, jsonAtLength(limit));
  if (mode === 'finite-oversize') return write(process.stdout, jsonAtLength(limit + 1));
  if (mode === 'open-oversize') {
    await write(process.stdout, jsonAtLength(limit + 1));
    return new Promise(() => {});
  }
  if (!['normal', 'stderr-duplex'].includes(mode)) throw new Error('unknown fixture mode');
  await write(process.stdout, Buffer.from('{"credentials":[{"id":"draft-a"}],"prune":[],"message":"ready"}'));
}

try {
  await run();
  clearTimeout(watchdog);
} catch {
  process.exit(98);
}

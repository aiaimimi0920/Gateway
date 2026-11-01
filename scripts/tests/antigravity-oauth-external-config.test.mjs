import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
test('Antigravity ships public installed-client defaults and only explicit runtime overrides', async () => {
  const constants = await readFile(path.join(root, 'src/protocol/antigravity.rs'), 'utf8');
  const loader = await readFile(path.join(root, 'src/protocol/antigravity/oauth_client.rs'), 'utf8');
  assert.equal(/pub const CLIENT_ID\s*:/.test(constants), true, 'public installed-client ID must be built in');
  assert.equal(/pub const CLIENT_SECRET\s*:/.test(constants), true, 'public installed-client parameter must be built in');
  assert.equal((constants.match(/gitleaks:allow/g) || []).length, 2, 'only the two public defaults have line-scoped exceptions');
  assert.equal(/Self::builtin/.test(loader), true, 'no configuration must select built-in defaults');
  assert.equal(/gateway_local_data/.test(loader), false, 'must not implicitly depend on a private data file');
  assert.equal(/(?:include_str|include_bytes|option_env|env)!/.test(loader), false, 'do not bake private overrides into the EXE');
});

import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import selfsigned from "selfsigned";
import { normalizeString } from "./gemini-canvas-browser-pool-input.mjs";

function getTlsAssetRoot() {
  const explicit = normalizeString(process.env.GEMINI_CANVAS_BROWSER_TLS_DIR);
  if (explicit) {
    return path.isAbsolute(explicit) ? explicit : path.resolve(process.cwd(), explicit);
  }
  return path.resolve(process.cwd(), "gateway/.runtime/gemini-canvas-browser-pool-tls");
}

function normalizeTlsAssetSlug(value) {
  return String(value || "local").replace(/[^a-z0-9._-]+/gi, "_");
}

export function loadOrCreateTlsCertificate(host) {
  const tlsRoot = getTlsAssetRoot();
  mkdirSync(tlsRoot, { recursive: true });
  const slug = normalizeTlsAssetSlug(host);
  const keyPath = path.join(tlsRoot, `${slug}.key.pem`);
  const certPath = path.join(tlsRoot, `${slug}.cert.pem`);

  if (existsSync(keyPath) && existsSync(certPath)) {
    return {
      key: readFileSync(keyPath, "utf8"),
      cert: readFileSync(certPath, "utf8"),
      keyPath,
      certPath,
      generated: false,
    };
  }

  const attrs = [{ name: "commonName", value: host }];
  const altNames = [{ type: 2, value: "localhost" }];
  if (/^\d{1,3}(\.\d{1,3}){3}$/.test(host)) {
    altNames.push({ type: 7, ip: host });
  } else {
    altNames.push({ type: 2, value: host });
  }
  altNames.push({ type: 7, ip: "127.0.0.1" });
  const generated = selfsigned.generate(attrs, {
    algorithm: "sha256",
    days: 30,
    keySize: 2048,
    extensions: [{ name: "subjectAltName", altNames }],
  });
  writeFileSync(keyPath, generated.private, "utf8");
  writeFileSync(certPath, generated.cert, "utf8");
  return {
    key: generated.private,
    cert: generated.cert,
    keyPath,
    certPath,
    generated: true,
  };
}

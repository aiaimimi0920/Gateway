/** Display-only recovery of legacy UTF-8 labels decoded as Latin-1, sometimes twice.
 * Never use for IDs, secrets, model names, raw logs or values written back to config.
 */
export function displayLabel(value: string): string {
  if (value.length > 4096) return value;
  let recovered = value;
  for (let round = 0; round < 3; round++) {
    if (!/[\u00c2-\u00f4][\u0080-\u00bf]/u.test(recovered)
      || [...recovered].some((char) => char.charCodeAt(0) > 255)) break;
    try {
      const bytes = Uint8Array.from(recovered, (char) => char.charCodeAt(0));
      recovered = new TextDecoder("utf-8", { fatal: true, ignoreBOM: true }).decode(bytes);
    } catch { return value; }
  }
  // Do not expose a partially recovered control sequence or guess missing characters.
  return /[\u0000-\u001f\u007f-\u009f\ufffd]/u.test(recovered) ? value : recovered;
}

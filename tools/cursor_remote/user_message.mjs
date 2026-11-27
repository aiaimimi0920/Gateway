// Pinned SelectedContext/SelectedImage mapping. Images are raw protobuf bytes,
// not URLs, prompt text, blob references or an invitation to enable AGENT mode.
import { randomUUID } from "node:crypto";
import { field } from "./protobuf_wire.mjs";
import { ProbeError } from "./transport.mjs";

const maxImage = 1024 * 1024;
const fail = () => { throw new ProbeError("invalid_inline_image"); };
export function validateImages(images = []) {
  if (!Array.isArray(images) || images.length > 4) fail();
  let total = 0;
  return images.map((image) => {
    if (!image || typeof image !== "object" || Array.isArray(image) ||
        Object.keys(image).some((key) => !["mime", "data"].includes(key)) ||
        !["image/png", "image/jpeg"].includes(image.mime) ||
        typeof image.data !== "string" || !image.data.length ||
        image.data.length > Math.ceil(maxImage / 3) * 4) fail();
    const bytes = Buffer.from(image.data, "base64");
    // Buffer.from alone accepts whitespace and malformed padding. Round-trip
    // equality enforces the same canonical STANDARD base64 contract as Rust.
    if (!bytes.length || bytes.length > maxImage || bytes.toString("base64") !== image.data ||
        (total += bytes.length) > 2 * maxImage) fail();
    const magic = image.mime === "image/png" ? Buffer.from([137, 80, 78, 71, 13, 10, 26, 10])
      : Buffer.from([255, 216, 255]);
    if (!bytes.subarray(0, magic.length).equals(magic)) fail();
    return { mime: image.mime, bytes };
  });
}
export function userMessage(prompt, mode, images) {
  const selected = validateImages(images).map(({ mime, bytes }) =>
    field(1, Buffer.concat([field(2, randomUUID()), field(7, mime), field(8, bytes)])));
  return Buffer.concat([field(1, prompt), field(2, randomUUID()),
    ...(selected.length ? [field(3, Buffer.concat(selected))] : []), field(4, mode)]);
}

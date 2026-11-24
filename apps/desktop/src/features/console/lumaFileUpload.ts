import { LUMA_FILE_LIMIT, LUMA_INLINE_FILE_LIMIT } from "../../api/console/luma-files";

export type LumaUpload = { action: "upload"; file: File; purpose: "input" | "reference"; expiresAt?: string; userId?: string };

export function validUpload(upload: LumaUpload): boolean {
  const f = upload.file;
  return !!f.size && f.size <= LUMA_FILE_LIMIT && !!f.name && new TextEncoder().encode(f.name).length <= 255
    && !/[\x00-\x1f\x7f/\\"]/.test(f.name)
    && (!upload.userId || (new TextEncoder().encode(upload.userId).length <= 256 && !/[\x00-\x1f\x7f]/.test(upload.userId)))
    && (!upload.expiresAt || (upload.expiresAt.length <= 64 && Number.isFinite(Date.parse(upload.expiresAt)) && Date.parse(upload.expiresAt) > Date.now()))
    && (!presigned(upload) || /^(image|video)\/[a-zA-Z0-9.+-]+$/.test(f.type));
}

export function presigned(upload: LumaUpload): boolean {
  return upload.file.size > LUMA_INLINE_FILE_LIMIT || !!upload.expiresAt || !!upload.userId;
}

export function uploadForm(upload: LumaUpload, binding: { providerId: string; credentialId: string; expectedRevision: string }): FormData {
  const form = new FormData();
  form.append("metadata", JSON.stringify({ ...binding, upload: { filename: upload.file.name, mime_type: upload.file.type,
    size_bytes: upload.file.size, purpose: upload.purpose,
    ...(upload.expiresAt ? { expires_at: new Date(upload.expiresAt).toISOString() } : {}),
    ...(upload.userId ? { user_id: upload.userId } : {}),
  } }));
  form.append("file", upload.file);
  return form;
}

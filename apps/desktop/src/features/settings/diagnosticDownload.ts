import type { GatewayDownload } from "./gatewayDiagnosticsApi";

export function downloadGatewayExport(download: GatewayDownload) {
  const url = URL.createObjectURL(new Blob([download.content], { type: download.mimeType }));
  const anchor = document.createElement("a");
  anchor.href = url; anchor.download = download.fileName;
  document.body.append(anchor);
  try { anchor.click(); } finally { anchor.remove(); window.setTimeout(() => URL.revokeObjectURL(url), 1000); }
}

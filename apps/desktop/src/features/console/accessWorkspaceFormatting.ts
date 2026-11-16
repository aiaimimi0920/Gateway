import type { Tone } from "@neuro/ui";

const PLACEHOLDER = "—";

/** Compact local time for key activity; the ledger tooltip retains the full timestamp. */
export function formatRecentKeyUse(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return PLACEHOLDER;
  const pad = (part: number) => String(part).padStart(2, "0");
  return `${pad(date.getMonth() + 1)}/${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

export function formatCount(value: number | null | undefined): string {
  if (value === null || value === undefined || !Number.isFinite(value)) {
    return PLACEHOLDER;
  }
  return new Intl.NumberFormat("en-US").format(value);
}

export function formatTimestamp(value: string | null | undefined): string {
  if (!value) {
    return PLACEHOLDER;
  }
  const parsed = new Date(value);
  if (Number.isNaN(parsed.getTime())) {
    return value;
  }
  return parsed.toLocaleString();
}

export function formatText(value: string | null | undefined): string {
  return value && value.trim().length > 0 ? value : PLACEHOLDER;
}

export function accessStatusTone(status: string | null | undefined): Tone {
  const normalized = (status ?? "").toLowerCase();
  if (normalized === "active" || normalized === "enabled") {
    return "success";
  }
  if (normalized === "revoked" || normalized === "disabled" || normalized === "expired") {
    return "danger";
  }
  if (normalized === "pending" || normalized === "suspended") {
    return "warning";
  }
  return "neutral";
}

export function maskSecret(value: string): string {
  if (value.length <= 8) {
    return "*".repeat(value.length);
  }
  return `${value.slice(0, 4)}${"*".repeat(value.length - 8)}${value.slice(-4)}`;
}

export { PLACEHOLDER };

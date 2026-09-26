const PLACEHOLDER = "—";

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

export function statusBadgeClass(status: string | null | undefined): string {
  const normalized = (status ?? "").toLowerCase();
  if (normalized === "active" || normalized === "enabled") {
    return "nt-badge nt-badge--success";
  }
  if (normalized === "revoked" || normalized === "disabled" || normalized === "expired") {
    return "nt-badge nt-badge--danger";
  }
  if (normalized === "pending" || normalized === "suspended") {
    return "nt-badge nt-badge--warning";
  }
  return "nt-badge";
}

export function maskSecret(value: string): string {
  if (value.length <= 8) {
    return "*".repeat(value.length);
  }
  return `${value.slice(0, 4)}${"*".repeat(value.length - 8)}${value.slice(-4)}`;
}

export { PLACEHOLDER };

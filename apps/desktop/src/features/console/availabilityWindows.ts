import type { AggregatedSuccessWindow } from "./providerCardMetrics";

export const AVAILABILITY_WINDOW_MS = 30 * 60 * 1000;
export const RECENT_AVAILABILITY_WINDOW_LIMIT = 8;

function timeOnlyMinutes(label: string): number | null {
  const match = /^(\d{1,2}):(\d{2})$/.exec(label);
  if (!match) return null;
  const hours = Number(match[1]);
  const minutes = Number(match[2]);
  return hours < 24 && minutes < 60 ? hours * 60 + minutes : null;
}

function clock(minutes: number): string {
  const wrapped = (minutes + 1440) % 1440;
  return `${String(Math.floor(wrapped / 60)).padStart(2, "0")}:${String(wrapped % 60).padStart(2, "0")}`;
}

/** Keep a bounded, contiguous timeline ending at the latest recorded bucket.
 * Missing buckets mean zero calls, not failed calls. Never subdivide counts. */
export function recentAvailabilityWindows(
  windows: readonly AggregatedSuccessWindow[],
  limit = RECENT_AVAILABILITY_WINDOW_LIMIT,
): AggregatedSuccessWindow[] {
  const count = Number.isFinite(limit) ? Math.min(96, Math.max(1, Math.floor(limit))) : RECENT_AVAILABILITY_WINDOW_LIMIT;
  if (!windows.length) return [];
  const timeOnly = timeOnlyMinutes(windows[0].label) !== null;
  const entries = windows.map((window) => ({
    window,
    start: timeOnly ? (timeOnlyMinutes(window.label) ?? NaN) * 60_000 : Date.parse(window.label),
  }));
  // Legacy/non-time labels may still be displayed, but cannot justify fake time buckets.
  if (entries.some(({ start }) => !Number.isFinite(start) || start % AVAILABILITY_WINDOW_MS !== 0)) {
    return windows.slice(-count);
  }
  entries.sort((left, right) => left.start - right.start);
  const end = entries[entries.length - 1].start;
  const start = Math.max(entries[0].start, end - (count - 1) * AVAILABILITY_WINDOW_MS);
  const byStart = new Map(entries.map((entry) => [entry.start, entry.window]));
  const result: AggregatedSuccessWindow[] = [];
  for (let bucket = start; bucket <= end; bucket += AVAILABILITY_WINDOW_MS) {
    result.push(byStart.get(bucket) ?? {
      label: timeOnly ? clock(bucket / 60_000) : new Date(bucket).toISOString().replace(".000Z", "Z"),
      success: 0, requests: 0, rate: null,
    });
  }
  return result;
}

export function formatAvailabilityRange(label: string): string {
  const minutes = timeOnlyMinutes(label);
  if (minutes !== null) return `${clock(minutes)}–${clock(minutes + 30)}`;
  const start = Date.parse(label);
  if (!Number.isFinite(start)) return label;
  const localClock = (timestamp: number) => {
    const date = new Date(timestamp);
    return clock(date.getHours() * 60 + date.getMinutes());
  };
  return `${localClock(start)}–${localClock(start + AVAILABILITY_WINDOW_MS)}`;
}

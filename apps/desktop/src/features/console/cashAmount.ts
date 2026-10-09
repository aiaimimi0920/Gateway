import type { ConsoleAccessKeyBalance } from "../../api/contracts";

const SCALE = 1_000_000n;

/** Parse the entered decimal, not a floating-point product of dollars and 1e6. */
export function parseCashMicros(value: string): number | null {
  const match = /^(\d+)(?:\.(\d{1,6}))?$/.exec(value);
  if (!match || value.length > 30) return null;
  const micros =
    BigInt(match[1]) * SCALE + BigInt((match[2] ?? "").padEnd(6, "0"));
  return micros <= BigInt(Number.MAX_SAFE_INTEGER) ? Number(micros) : null;
}

export function formatCashMicros(value: number | bigint): string {
  if (typeof value === "number" && !Number.isSafeInteger(value)) return "—";
  const micros = BigInt(value);
  const magnitude = micros < 0 ? -micros : micros;
  const fraction = String(magnitude % SCALE)
    .padStart(6, "0")
    .replace(/0+$/, "");
  return `${micros < 0 ? "-" : ""}${magnitude / SCALE}${fraction ? `.${fraction}` : ""}`;
}

export function cashRemaining(
  cash: NonNullable<ConsoleAccessKeyBalance["cash"]>,
): bigint {
  return (
    BigInt(cash.totalMicros) -
    BigInt(cash.spentMicros) -
    BigInt(cash.reservedMicros)
  );
}

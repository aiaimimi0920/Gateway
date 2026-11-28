/** Empty means no hard ceiling; never interpret it as zero or a provider-wide default. */
export function parseAccountConcurrencyLimit(text: string | undefined): number | undefined {
  const value = (text ?? "").trim();
  if (!value) return undefined;
  if (!/^[1-9]\d*$/.test(value)) throw new Error("账号并发上限必须是 1–1000 的整数。");
  const limit = Number(value);
  if (!Number.isSafeInteger(limit) || limit > 1000) throw new Error("账号并发上限必须是 1–1000 的整数。");
  return limit;
}

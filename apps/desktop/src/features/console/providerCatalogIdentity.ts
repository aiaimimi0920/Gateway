/** Templates describe provider types; each creation allocates an independent identity. */
export function availableCatalogId(base: string, existing: readonly string[]): string {
  const occupied = new Set(existing.map((id) => id.trim()));
  if (!occupied.has(base)) return base;
  for (let suffix = 2; ; suffix += 1) {
    const candidate = `${base}-${suffix}`;
    if (!occupied.has(candidate)) return candidate;
  }
}

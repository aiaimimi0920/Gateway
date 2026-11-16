/** A running card action can disable its opener; keep keyboard focus on that card. */
export function resolveProviderCardDialogFocus(opener: HTMLElement | null): HTMLElement | null {
  if (opener?.matches(":disabled")) {
    return opener.closest("article")?.querySelector<HTMLButtonElement>(".nt-provider-card__back .nt-provider-card__flip") ?? null;
  }
  return opener;
}

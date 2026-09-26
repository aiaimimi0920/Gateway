export type ProviderOwnedAccount = {
  providerId: string;
};

/** Builds provider buckets in one pass while preserving the account display order. */
export function indexAccountsByProvider<T extends ProviderOwnedAccount>(
  accounts: readonly T[],
): Map<string, T[]> {
  const accountsByProvider = new Map<string, T[]>();

  for (const account of accounts) {
    const { providerId } = account;
    const providerAccounts = accountsByProvider.get(providerId);
    if (providerAccounts) {
      providerAccounts.push(account);
    } else {
      accountsByProvider.set(providerId, [account]);
    }
  }

  return accountsByProvider;
}

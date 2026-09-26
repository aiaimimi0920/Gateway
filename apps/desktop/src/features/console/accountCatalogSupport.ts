export type ProviderVendorMetadata = {
  bucketKey: string;
  key: string;
  name: string;
};

export function optionalString(record: Record<string, unknown>, key: string): string | null {
  const value = record[key];
  return typeof value === "string" && value.trim().length > 0 ? value.trim() : null;
}

function normalizeBucketKey(value: string): string {
  return value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
}

export function providerVendorMetadata(
  provider: Record<string, unknown>,
  providerId: string,
  providerLabel: string,
  keyField: "vendor_key" | "vendorKey",
  nameField: "vendor_name" | "vendorName",
): ProviderVendorMetadata {
  const explicitKey = optionalString(provider, keyField);
  const explicitName = optionalString(provider, nameField);
  if (explicitKey) {
    return {
      bucketKey: `vendor:${normalizeBucketKey(explicitKey) || explicitKey.toLocaleLowerCase()}`,
      key: explicitKey,
      name: explicitName ?? explicitKey,
    };
  }
  if (explicitName) {
    const normalizedName = normalizeBucketKey(explicitName) || providerId;
    return {
      bucketKey: `vendor-name:${normalizedName}`,
      key: normalizedName,
      name: explicitName,
    };
  }
  return {
    bucketKey: `provider:${providerId}`,
    key: providerId,
    name: providerLabel,
  };
}

export function hostLabelFromUrl(url: string | null): string | null {
  if (!url) {
    return null;
  }
  try {
    return new URL(url).host;
  } catch {
    return url;
  }
}

// Cohesive declarative builder for onboarding standard API Key / OAuth / Header providers
// without duplicating origin allowlists, catalog URLs, and credential field definitions.
import type {
  ProviderCatalogCategory,
  ProviderCatalogTemplate,
  ProviderCompatibility,
} from "./providerCatalog";

export type DeclarativeCredentialField = {
  readonly key: string;
  readonly labelZh: string;
  readonly labelEn: string;
  readonly kind: "secret" | "text" | "select";
  readonly required: boolean;
  readonly placeholder?: string;
  readonly options?: readonly { readonly value: string; readonly labelZh: string; readonly labelEn: string }[];
};

export type DeclarativeProviderInput = {
  readonly id: string;
  readonly providerId?: string;
  readonly preset: string;
  readonly vendorKey: string;
  readonly vendorName: string;
  readonly descriptionZh: string;
  readonly descriptionEn: string;
  readonly category?: ProviderCatalogCategory;
  readonly compatibility?: ProviderCompatibility;
  readonly defaultBaseUrl: string;
  readonly allowedOrigins?: readonly string[];
  readonly modelsPath?: string | null;
  readonly supportedModels?: readonly string[];
  readonly extraFields?: readonly DeclarativeCredentialField[];
};

export type DeclarativeProviderBundle = {
  readonly template: ProviderCatalogTemplate;
  readonly modelsCatalogUrl: string | null;
  readonly credentialFields: readonly DeclarativeCredentialField[];
  readonly isApprovedBaseUrl: (candidate: string) => boolean;
};

function normalizeHttpsBase(raw: string): string | null {
  const trimmed = raw.trim();
  if (!trimmed) return null;
  let parsed: URL;
  try {
    parsed = new URL(trimmed);
  } catch {
    return null;
  }
  if (parsed.protocol !== "https:" || parsed.username || parsed.password || parsed.search || parsed.hash) {
    return null;
  }
  return trimmed.replace(/\/+$/, "");
}

const DEFAULT_API_KEY_FIELD: DeclarativeCredentialField = {
  key: "apiKey",
  labelZh: "API Key",
  labelEn: "API Key",
  kind: "secret",
  required: true,
};

/** Build a complete catalog template, strict HTTPS origin validator, and field schema in one declaration. */
export function defineDeclarativeProvider(input: DeclarativeProviderInput): DeclarativeProviderBundle {
  const canonicalDefault = normalizeHttpsBase(input.defaultBaseUrl);
  if (!canonicalDefault) {
    throw new Error(`Declarative provider '${input.id}' requires a clean HTTPS defaultBaseUrl`);
  }
  const allowSet = new Set(
    (input.allowedOrigins && input.allowedOrigins.length > 0
      ? input.allowedOrigins
      : [canonicalDefault]
    ).map((origin) => {
      const normalized = normalizeHttpsBase(origin);
      if (!normalized) throw new Error(`Invalid allowed origin '${origin}' in '${input.id}'`);
      return normalized;
    }),
  );

  const providerId = input.providerId ?? input.id;
  const template: ProviderCatalogTemplate = {
    id: input.id,
    providerId,
    labelZh: providerId,
    labelEn: providerId,
    descriptionZh: input.descriptionZh,
    descriptionEn: input.descriptionEn,
    category: input.category ?? "mainstream",
    compatibility: input.compatibility ?? "openai",
    preset: input.preset,
    vendorKey: input.vendorKey,
    vendorName: input.vendorName,
    baseUrl: canonicalDefault,
    supportedModels: input.supportedModels ?? [],
  };

  const modelsCatalogUrl =
    input.modelsPath === null || input.modelsPath === undefined
      ? null
      : `${canonicalDefault}/${input.modelsPath.replace(/^\/+/, "")}`;

  const credentialFields: readonly DeclarativeCredentialField[] = [
    DEFAULT_API_KEY_FIELD,
    ...(input.extraFields ?? []),
  ];

  return {
    template,
    modelsCatalogUrl,
    credentialFields,
    isApprovedBaseUrl: (candidate: string) => {
      const normalized = normalizeHttpsBase(candidate);
      return normalized !== null && allowSet.has(normalized);
    },
  };
}

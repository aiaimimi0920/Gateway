export type RouteAccountGroup = {
  id: string;
  name: string;
  description: string | null;
  billingMultiplier: number;
  enabled: boolean;
  notes: string | null;
  providerCredentialIds: string[];
};

export type RouteManagedAccount = {
  id: string;
  displayName: string;
  vendorKey: string;
  vendorName: string;
  providerId: string;
  providerLabel: string;
  providerPreset: string | null;
  baseUrl: string | null;
  hostLabel: string | null;
  mode: "credential" | "provider-default";
  enabled: boolean;
  supportedModels: string[];
  groupIds: string[];
  groupNames: string[];
};

export type RouteProviderBucket = {
  key: string;
  label: string;
  vendorKey: string;
  accountCount: number;
  providers: Array<{
    id: string;
    label: string;
    preset: string | null;
    baseUrl: string | null;
    accounts: RouteManagedAccount[];
  }>;
};

export type RouteAccountCatalog = {
  groups: RouteAccountGroup[];
  accounts: RouteManagedAccount[];
  providerBuckets: RouteProviderBucket[];
  ungroupedCount: number;
};

export type AccountMembershipFilter = "all" | "grouped" | "ungrouped";
export type AccountEnabledFilter = "all" | "enabled" | "disabled";

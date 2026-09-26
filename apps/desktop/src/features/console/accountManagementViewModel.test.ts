import { describe, expect, it } from "vitest";

import {
  buildAccountLedgerRows,
  buildRouteAccountCatalogFromSummary,
  buildCredentialGroupDirectory,
  buildGroupMemberCandidates,
  filterAccountLedgerRows,
  resolveGeminiLogicalChannel,
} from "./accountManagementViewModel";
import type { ProviderMetricsResolver } from "./providerCardMetrics";

describe("accountManagementViewModel", () => {
  const catalog = {
    groups: [
      {
        id: "group-vip",
        name: "VIP 分组",
        description: null,
        billingMultiplier: 1.5,
        enabled: true,
        notes: null,
        providerCredentialIds: ["acc-prod-1"],
      },
    ],
    accounts: [
      {
        id: "acc-prod-1",
        displayName: "生产账号 A",
        vendorKey: "openai",
        vendorName: "OpenAI",
        providerId: "managed-provider",
        providerLabel: "Managed OpenAI",
        providerPreset: "openai",
        baseUrl: "https://api.example.com/v1",
        hostLabel: "api.example.com",
        mode: "credential" as const,
        enabled: true,
        supportedModels: ["gpt-5.4"],
        groupIds: ["group-vip"],
        groupNames: ["VIP 分组"],
      },
      {
        id: "managed-provider::default",
        displayName: "Managed OpenAI 默认账号",
        vendorKey: "openai",
        vendorName: "OpenAI",
        providerId: "managed-provider",
        providerLabel: "Managed OpenAI",
        providerPreset: "openai",
        baseUrl: "https://api.example.com/v1",
        hostLabel: "api.example.com",
        mode: "provider-default" as const,
        enabled: false,
        supportedModels: ["gpt-5.4-mini"],
        groupIds: [],
        groupNames: [],
      },
    ],
    providerBuckets: [],
    ungroupedCount: 1,
  };

  it("builds ledger rows with provider, groups, and probe state", () => {
    const rows = buildAccountLedgerRows(catalog, {
      "acc-prod-1": {
        credentialId: "acc-prod-1",
        providerId: "managed-provider",
        status: "passed",
        message: "Credential connectivity probe passed.",
        checkedAt: "2026-07-29T10:30:00Z",
      },
    });

    expect(rows[0]).toMatchObject({
      accountId: "acc-prod-1",
      providerLabel: "Managed OpenAI",
      vendorLabel: "OpenAI",
      groupLabels: ["VIP 分组"],
      enabled: true,
      probeStatus: "passed",
      verificationStatus: "not-tested",
    });
    expect(rows[1]).toMatchObject({
      accountId: "managed-provider::default",
      mode: "provider-default",
      groupLabels: [],
      probeStatus: "not-tested",
      verificationStatus: "not-tested",
    });
  });

  it("filters ledger rows by search, grouping, provider, and enabled state", () => {
    const rows = buildAccountLedgerRows(catalog, {});
    const filtered = filterAccountLedgerRows(rows, {
      query: "生产账号 A",
      membership: "grouped",
      enabled: "enabled",
      groupId: "group-vip",
      providerKey: "openai",
    });

    expect(filtered).toHaveLength(1);
    expect(filtered[0]?.accountId).toBe("acc-prod-1");
  });

  it("maps internal Gemini providers to three operator-visible channels", () => {
    expect(resolveGeminiLogicalChannel("gemini-web-secondary", "gemini-web-chat-modular"))
      .toMatchObject({ key: "logical:gemini", label: "Gemini" });
    expect(resolveGeminiLogicalChannel("gemini-canvas-chat", "gemini-canvas-chat"))
      .toMatchObject({ key: "logical:gemini", label: "Gemini" });
    expect(resolveGeminiLogicalChannel("gemini-business", "gemini-business"))
      .toMatchObject({ key: "logical:gemini-business", label: "Gemini Business" });
    expect(resolveGeminiLogicalChannel("gemini-canvas", "gemini-canvas-program-relay"))
      .toMatchObject({ key: "logical:gemini-canvas", label: "Gemini Canvas" });
  });

  it("drops summary accounts whose provider was deleted from the active inventory", () => {
    const summary = {
      routeConfigRevision: "r45-current",
      source: "redis",
      accountGroups: [],
      providers: [
        {
          id: "active-provider",
          label: "Active provider",
          accountIds: ["active-account"],
          supportedModels: ["active-model"],
        },
      ],
      accounts: [
        {
          id: "active-account",
          displayName: "Active account",
          providerId: "active-provider",
          providerLabel: "Active provider",
          mode: "credential",
          enabled: true,
          supportedModels: ["active-model"],
          groupIds: [],
        },
        {
          id: "legacy-account",
          displayName: "Deleted legacy account",
          providerId: "gemini-canvas-legacy-media",
          providerLabel: "Gemini Canvas legacy media",
          mode: "credential",
          enabled: true,
          supportedModels: ["legacy-model"],
          groupIds: [],
        },
      ],
    };

    const result = buildRouteAccountCatalogFromSummary(summary);

    expect(result.accounts.map((account) => account.id)).toEqual(["active-account"]);
    expect(result.providerBuckets.flatMap((bucket) => bucket.providers.map((provider) => provider.id)))
      .toEqual(["active-provider"]);
  });

  it("filters all providers that belong to one logical Gemini channel", () => {
    const rows = [
      {
        ...buildAccountLedgerRows(catalog, {})[0]!,
        accountId: "gemini-chat-account",
        providerId: "gemini-canvas-chat",
        providerLabel: "gemini-canvas-chat",
        providerPreset: "gemini-canvas-chat",
      },
      {
        ...buildAccountLedgerRows(catalog, {})[0]!,
        accountId: "gemini-web-account",
        providerId: "gemini-web-secondary",
        providerLabel: "Gemini Web /u/1/",
        providerPreset: "gemini-web-chat-modular",
      },
      {
        ...buildAccountLedgerRows(catalog, {})[0]!,
        accountId: "gemini-canvas-account",
        providerId: "gemini-canvas",
        providerLabel: "gemini-canvas",
        providerPreset: "gemini-canvas-program-relay",
      },
    ];

    const filtered = filterAccountLedgerRows(rows, {
      query: "",
      membership: "all",
      enabled: "all",
      groupId: "all",
      providerKey: "logical:gemini",
    });

    expect(filtered.map((row) => row.accountId)).toEqual([
      "gemini-chat-account",
      "gemini-web-account",
    ]);
  });

  it("builds group directory items and candidate rows without provider buckets", () => {
    const directory = buildCredentialGroupDirectory(catalog, [
      {
        id: "draft-group-vip",
        groupId: "group-vip",
        name: "VIP 分组",
        description: "",
        billingMultiplier: "1.5",
        enabled: true,
        notes: "",
        providerCredentialIds: ["acc-prod-1"],
      },
    ]);
    const candidates = buildGroupMemberCandidates(catalog.accounts, {
      selectedCredentialIds: ["acc-prod-1"],
      query: "Managed OpenAI",
      mode: "all",
    });

    expect(directory[0]).toMatchObject({
      rowId: "draft-group-vip",
      groupId: "group-vip",
      memberCount: 1,
      providerLabels: ["Managed OpenAI"],
    });
    // Display-name order varies with the host locale.
    expect(candidates).toHaveLength(2);
    expect(candidates.find((candidate) => candidate.accountId === "acc-prod-1")).toMatchObject({
      accountId: "acc-prod-1",
      selected: true,
      providerLabel: "Managed OpenAI",
    });
    expect(candidates.find((candidate) => candidate.accountId === "managed-provider::default"))
      .toMatchObject({
        accountId: "managed-provider::default",
        selected: false,
        providerLabel: "Managed OpenAI",
      });
  });

  it("rolls member usage up per group and ignores non-members", () => {
    const [directory] = buildCredentialGroupDirectory(
      catalog,
      [
        {
          id: "draft-group-vip",
          groupId: "group-vip",
          name: "VIP 分组",
          description: "",
          billingMultiplier: "1.5",
          enabled: true,
          notes: "",
          providerCredentialIds: ["acc-prod-1"],
        },
      ],
      new Map([
        [
          "acc-prod-1",
          {
            concurrencyUsed: 4,
            concurrencyTotal: 10,
            usageWindowBadges: ["120 req"],
            upstreamCost: 1.5,
            userCost: 3,
            successWindows: [{ label: "10:00", success: 9, requests: 10 }],
          },
        ],
        [
          "acc-prod-2",
          {
            concurrencyUsed: 99,
            concurrencyTotal: 99,
            usageWindowBadges: ["9,000 req"],
            upstreamCost: 50,
            userCost: 80,
            successWindows: [{ label: "10:00", success: 0, requests: 100 }],
          },
        ],
      ]),
    );

    expect(directory?.metrics).toMatchObject({
      concurrency: { used: 4, total: 10 },
      upstreamCost: 1.5,
      platformRevenue: 3,
      requests: 120,
      successRate: 0.9,
    });
  });

  it("leaves group metrics null when no per-account usage is supplied", () => {
    const [directory] = buildCredentialGroupDirectory(catalog, [
      {
        id: "draft-group-vip",
        groupId: "group-vip",
        name: "VIP 分组",
        description: "",
        billingMultiplier: "1.5",
        enabled: true,
        notes: "",
        providerCredentialIds: ["acc-prod-1"],
      },
    ]);

    expect(directory?.metrics).toBeNull();
  });

  it("counts the provider's whole pool on a scope tab, not just the member credentials", () => {
    const [directory] = buildCredentialGroupDirectory(catalog, [
      {
        id: "draft-group-vip",
        groupId: "group-vip",
        name: "VIP 分组",
        description: "",
        billingMultiplier: "1.5",
        enabled: true,
        notes: "",
        providerCredentialIds: ["acc-prod-1"],
      },
    ]);

    expect(directory?.providerScopes).toEqual([
      {
        providerId: "managed-provider",
        providerLabel: "Managed OpenAI",
        accountCount: 1,
        modelCount: 1,
        // Both catalog accounts belong to this provider, while only one is a
        // member of the group.
        providerAccountCount: 2,
      },
    ]);
  });

  it("takes group and per-model numbers from the provider-account resolver rather than the member cards", () => {
    const resolverCalls: {
      providerAccounts: string[][];
      providerAccountModel: [string, string][];
      billingMultipliers: (number | undefined)[];
    } = { providerAccounts: [], providerAccountModel: [], billingMultipliers: [] };
    const resolver: ProviderMetricsResolver = {
      providerAccounts: (providerAccountIds, options) => {
        resolverCalls.providerAccounts.push([...providerAccountIds]);
        resolverCalls.billingMultipliers.push(options?.billingMultiplier);
        return {
          concurrencyUsed: 6,
          concurrencyTotal: 20,
          requestCount: 400,
          upstreamCost: 8,
          userCost: 12,
          successWindows: [{ label: "10:00", success: 380, requests: 400 }],
        };
      },
      providerAccountModel: (providerAccountId, model) => {
        resolverCalls.providerAccountModel.push([providerAccountId, model]);
        return {
          concurrencyUsed: 2,
          concurrencyTotal: 5,
          requestCount: 100,
          upstreamCost: 2,
          userCost: 3,
          successWindows: [{ label: "10:00", success: 90, requests: 100 }],
        };
      },
    };

    const [directory] = buildCredentialGroupDirectory(
      catalog,
      [
        {
          id: "draft-group-vip",
          groupId: "group-vip",
          name: "VIP 分组",
          description: "",
          billingMultiplier: "1.5",
          enabled: true,
          notes: "",
          providerCredentialIds: ["acc-prod-1"],
        },
      ],
      // The per-account map is deliberately smaller than the resolver's answer, so
      // a card that summed its members would report 1/2 instead of 6/20.
      new Map([
        [
          "acc-prod-1",
          { concurrencyUsed: 1, concurrencyTotal: 2, requestCount: 5, upstreamCost: 1, userCost: 2 },
        ],
      ]),
      resolver,
    );

    expect(resolverCalls.providerAccounts).toEqual([["managed-provider"]]);
    expect(resolverCalls.billingMultipliers).toEqual([1.5]);
    expect(directory?.metrics).toMatchObject({
      concurrency: { used: 6, total: 20 },
      requests: 400,
      upstreamCost: 8,
      platformRevenue: 12,
      successRate: 0.95,
    });
    expect(resolverCalls.providerAccountModel).toEqual([["managed-provider", "gpt-5.4"]]);
    expect(directory?.modelScopes).toMatchObject([
      {
        model: "gpt-5.4",
        members: [{ accountId: "acc-prod-1", providerId: "managed-provider" }],
        providerMetrics: [{ providerId: "managed-provider", metrics: { requestCount: 100 } }],
      },
    ]);
  });
});

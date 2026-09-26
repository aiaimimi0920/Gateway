--
-- Gateway standalone bootstrap rows.
--
-- 001-gateway-schema.sql only creates tables. When Gateway runs next to the
-- Platform control plane, Platform inserts the tenant/project/route-policy rows
-- for every benefit project. A standalone Gateway stack has no such control
-- plane, so relay requests authenticated with the shared `GATEWAY_API_KEY` land
-- on `GATEWAY_DEFAULT_PROJECT_ID` (default `platform-default-project`) and fail
-- with `404 AI gateway project 不存在` unless that project exists.
--
-- These rows are the minimum a standalone deployment needs: one tenant, the
-- default project, the project's default route policy, and the two synthetic
-- API key identities the gateway attributes shared-secret and dev-mode traffic
-- to. The route-policy config below is the same document
-- `normalize_route_policy_config(GatewayRoutePolicyConfigInput::default())`
-- produces, so it must be re-captured if those Rust defaults change.
--
-- Override `GATEWAY_DEFAULT_PROJECT_ID` if you seed a different project id.
--

INSERT INTO public.gateway_tenants (
  id,
  slug,
  display_name,
  status,
  owner_user_id,
  source_kind,
  source_key,
  created_at,
  updated_at
) VALUES (
  'platform-default-tenant',
  'platform-default',
  'Gateway Default Tenant',
  'active',
  NULL,
  'gateway_bootstrap',
  'gateway_bootstrap:platform-default-tenant',
  now(),
  now()
) ON CONFLICT (id) DO NOTHING;

INSERT INTO public.gateway_projects (
  id,
  tenant_id,
  slug,
  display_name,
  status,
  source_kind,
  source_key,
  default_route_policy_id,
  created_at,
  updated_at
) VALUES (
  'platform-default-project',
  'platform-default-tenant',
  'platform-default-project',
  'Gateway Default Project',
  'active',
  'gateway_bootstrap',
  'gateway_bootstrap:platform-default-project',
  'platform-default-route-policy',
  now(),
  now()
) ON CONFLICT (id) DO NOTHING;

-- Request-audit identities for the two auth modes that have no operator-issued
-- key: `GATEWAY_API_KEY` shared-secret auth (`gateway-key`) and the no-auth dev
-- fallback (`dev-mode`). `gateway_request_audits` requires one of
-- api_key_id/user_credential_id/access_key_id and enforces a foreign key on
-- api_key_id, so without these rows every relay request logs
-- `failed to create request audit ... 引用的资源不存在` and the console's live
-- concurrency and success-rate panels stay empty.
--
-- `gateway_api_keys` stores no secret material at all: relay authentication runs
-- against `gateway_access_keys` and an HMAC over `GATEWAY_API_KEY_SECRET`, so
-- these rows grant nothing. Their `system` status also keeps them out of the
-- `status = 'active'` lookups that issue and rotate a project's API key.
INSERT INTO public.gateway_api_keys (
  id,
  project_id,
  name,
  status,
  rotated_from_api_key_id,
  revoked_at,
  revoked_by_user_id,
  revoke_reason,
  created_at,
  updated_at
) VALUES
  (
    'gateway-key',
    'platform-default-project',
    'Shared GATEWAY_API_KEY',
    'system',
    NULL,
    NULL,
    NULL,
    NULL,
    now(),
    now()
  ),
  (
    'dev-mode',
    'platform-default-project',
    'Dev mode (no auth configured)',
    'system',
    NULL,
    NULL,
    NULL,
    NULL,
    now(),
    now()
  )
ON CONFLICT (id) DO NOTHING;

INSERT INTO public.gateway_route_policies (
  id,
  project_id,
  name,
  is_default,
  enabled,
  config,
  created_at,
  updated_at
) VALUES (
  'platform-default-route-policy',
  'platform-default-project',
  'default',
  true,
  true,
  '{
    "stickySessions": true,
    "preStreamFallbackEnabled": true,
    "selectionStrategy": "weighted_random",
    "providerLoadAwareRoutingEnabled": true,
    "maxConcurrentRequests": null,
    "providerMaxConcurrentRequests": null,
    "rateLimitEnforcementVersion": null,
    "rateLimitWindowSeconds": null,
    "rateLimitMaxRequests": null,
    "apiKeyRateLimit": null,
    "modelRateLimits": null,
    "endpointRateLimits": null,
    "providerAttemptRateLimit": null,
    "circuitBreakerThreshold": 3,
    "circuitBreakerCooldownSeconds": 60,
    "allowedProviderAccountIds": null,
    "allowedProtocolFamilies": null,
    "allowedModelIds": null,
    "blockedModelIds": null,
    "maxRequestBodyBytes": null,
    "streamIdleTimeoutSeconds": null,
    "totalRequestTimeoutSeconds": null,
    "maxStreamHeartbeatGapSeconds": null,
    "routingAnomalyAutoRemediation": null,
    "rateLimitHotspotAutoRemediation": null,
    "fallbackHttpStatuses": [408, 425, 429, 500, 502, 503, 504],
    "fallbackErrorCodes": [
      "ECONNRESET",
      "ECONNREFUSED",
      "ETIMEDOUT",
      "UND_ERR_CONNECT_TIMEOUT",
      "UND_ERR_HEADERS_TIMEOUT",
      "UND_ERR_BODY_TIMEOUT"
    ]
  }'::jsonb,
  now(),
  now()
) ON CONFLICT (id) DO NOTHING;

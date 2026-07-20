# Gateway Provider Evidence

Gateway provider evidence separates locally verifiable implementation facts from
third-party account, credential, browser, quota, region, and protocol state. A
provider line is not declared live only because its code compiles, and an
external provider gate is not automatically treated as a Gateway build failure.

The full matrix entry point is a source repository only tool:

```powershell
tools/run-gateway-line-evidence.ps1
```

It requires the source `Cargo.toml`, `src/`, and deployment verifier, so it is
deliberately excluded from portable releases. An extracted package contains the
standalone `scripts/invoke-gateway-live-provider-canary.ps1` canary. From the
package root, run it without `-AllowLiveProviderCalls` to verify the dry-run
contract without network access.

## Safety Contract

- The default mode is offline. It never invokes a live provider canary.
- Live calls require the explicit `-AllowLiveProviderCalls` switch.
- The runner delegates live HTTP execution and response classification to
  the Gateway-owned `scripts/invoke-gateway-live-provider-canary.ps1`; it does
  not implement a second provider client. The script is included in the
  portable release, while the monorepo-level script is only a compatibility
  wrapper.
- GET targets are delegated as GET without a request body. They are not
  rewritten to POST or classified as unsupported by the runner.
- Evidence JSON contains credential source labels, never API keys, cookies,
  bearer tokens, session values, or browser storage material.
- Ordinary CI should use offline mode. Live mode is an operator action against
  an isolated Gateway instance.
- Live route proof is disabled unless the server sets
  `GATEWAY_LIVE_ROUTE_PROOF_ENABLED=true`, and the individual request opts in
  with `X-Neuro-Gateway-Route-Proof-Request: v1`.

## Offline Modes

With no line selection, the runner checks representative official HTTP,
direct HTTP replay, browser-backed, search, media, and websocket lines. This
also covers every manifest-declared execution mode without making a live call:

```powershell
tools/run-gateway-line-evidence.ps1 -SkipCargo
```

`-SkipCargo` still runs the complete manifest validator, the focused line
verifier, the inventory generator, and the inventory validator. The resulting
line state is `metadata_only`: the source and manifest contract was checked,
but Cargo execution was intentionally not observed, so the runner makes no
compile or fixture claim.

Run one line or a comma-separated selection with focused Cargo tests:

```powershell
tools/run-gateway-line-evidence.ps1 `
  -LineId "anthropic-messages-official-model-api,xfyun-native-websocket-official-vendor-api" `
  -LibOnly
```

Run the complete offline matrix:

```powershell
tools/run-gateway-line-evidence.ps1 `
  -All `
  -LibOnly `
  -SharedCargoTargetDir target/provider-line-matrix
```

A focused verification that executes and passes its Cargo filters is recorded
as `fixture_passed`. A failed or skipped focused verification is recorded as
`metadata_only` with `gateway_verification/offline_focused_verification_failed`
or `offline_metadata_only` plus any available stdout and stderr artifact paths.

## Live Mode

Live mode requires all of the following:

- `-AllowLiveProviderCalls`
- `-GatewayBaseUrl`
- `GATEWAY_CANARY_API_KEY` or `-GatewayApiKey`
- `-TargetsPath`

Example:

```powershell
$env:GATEWAY_CANARY_API_KEY = $env:GATEWAY_CANARY_KEY
tools/run-gateway-line-evidence.ps1 `
  -AllowLiveProviderCalls `
  -GatewayBaseUrl "http://127.0.0.1:18080" `
  -TargetsPath "C:/secure/gateway-canary-targets.json" `
  -LineId "openrouter-openai-aggregator-api"
```

Do not place a literal key in a checked-in script or targets file. The Gateway
API key authenticates the canary to the local Gateway and is not written to the
provider evidence record. The runner passes it to the delegated canary through
the process environment, and the canary supplies curl from a short-lived header
file rather than exposing the value in a child command line.

### Live Route Proof Contract

The proof contract is deliberately narrow and applies only to the final HTTP
response for a normal request. It does not claim proof for WebSocket upgrades or
work performed by an upgraded connection task.

1. The isolated Gateway process sets
   `GATEWAY_LIVE_ROUTE_PROOF_ENABLED=true`.
2. The canary sends `X-Neuro-Gateway-Route-Proof-Request: v1` and a unique
   `X-Request-Id`.
3. After a provider candidate succeeds, Gateway derives the canonical manifest
   line and records only that non-secret identity.
4. For a final HTTP 2xx response, Gateway emits
   `X-Neuro-Gateway-Route-Proof: v1` and
   `X-Neuro-Gateway-Provider-Line: <canonical-line>`.
5. The response `X-Request-Id` must match the request, and the observed line
   must match `expected_provider_line`. When that field is omitted, the canary
   uses the target's declared `provider_line`; a target with neither field
   fails before HTTP execution.

Gateway removes any same-named proof headers supplied by an upstream or binary
passthrough before injecting its own values. It does not expose provider account
IDs, credential IDs, provider labels, credential references, platform access
IDs, API keys, cookies, JWTs, sessions, adapter names, or protocol profiles.

A 2xx response without a complete matching proof is a failure classified as
`route_proof_missing_or_mismatch`; it is never promoted to `live_passed`. Proof
headers are not emitted for non-2xx responses.

The evidence runner applies a second strict check before promoting a result:
`route_proof` must be an object with source exactly
`gateway_response_headers_v1`; result, observed, and proof provider lines must
all equal the selected canonical line; proof and result request IDs must be
non-empty and identical; and `http_status` must be 2xx.

Targets may be a JSON array or an object with a `targets` array. The
`credential_source` field is a label such as `vault:openrouter-primary`, not
credential material:

```json
{
  "targets": [
    {
      "name": "openrouter-conversation",
      "provider_line": "openrouter-openai-aggregator-api",
      "expected_provider_line": "openrouter-openai-aggregator-api",
      "credential_source": "vault:openrouter-primary",
      "endpoint": "/v1/chat/completions",
      "method": "POST",
      "model": "operator-approved-model",
      "body": {
        "model": "operator-approved-model",
        "messages": [{ "role": "user", "content": "gateway live canary" }],
        "max_tokens": 1
      },
      "lease_cooling_ready": true,
      "model_health_ready": true
    }
  ]
}
```

Browser-backed targets can also provide readiness observations:

```json
{
  "remote_executor_ready": true,
  "local_browser_ready": false,
  "session_material_ready": true,
  "credential_refresh_ready": true,
  "execution_mode": "browser_backed"
}
```

The runner filters the targets to the selected provider lines before delegating
to the canary. A line without a matching live target is recorded as
`credential_missing`; it is not silently treated as live.
In a multi-target run, a failed target does not invalidate another line that
returned its own complete strict proof.

## Evidence States

| State | Meaning |
| --- | --- |
| `compiled` | An operator supplied explicit evidence that the relevant compile contract was observed. This state is not emitted by `-SkipCargo`. |
| `metadata_only` | Manifest, feature, source, and focused line metadata are present, but Cargo execution was skipped or did not produce a compile/fixture proof. |
| `fixture_passed` | The focused offline line verifier and Cargo filters passed. No provider call was made. |
| `live_passed` | An explicitly enabled live canary passed for the provider line. |
| `external_gate` | The live attempt reached a classified third-party or environmental gate. |
| `credential_missing` | No usable target/credential was available, or the canary classified authentication material as unavailable. |
| `known_unsupported` | The capability is intentionally unsupported and documented as such. |

Every persisted record includes a UTC timestamp and a classification with
`failureClass`, `failureCode`, and `message`. The inventory generator merges the
record into `docs/provider-inventory.json`, and the evidence validator checks
state ordering, manifest coverage, timestamps, classifications, and secret-like
fields.

## External Failure Classes

External failures must be classified rather than collapsed into a generic
provider failure:

| Failure class | Evidence state | Typical condition |
| --- | --- | --- |
| `credential` | `credential_missing` | Missing, expired, revoked, or rejected credential/session material. |
| `quota` | `external_gate` | Account quota exhausted or provider rate limit reached. |
| `challenge` | `external_gate` | CAPTCHA, anti-bot, device verification, or browser challenge. |
| `account` | `external_gate` | Account disabled, unverified, subscription-gated, or lacking entitlement. |
| `region` | `external_gate` | Provider or model unavailable from the current region. |
| `upstream_protocol` | `external_gate` | Provider response or request contract changed outside the Gateway release. |
| `upstream` | `external_gate` | Provider or Gateway-facing upstream returned a server failure. |
| `network` | `external_gate` | DNS, connect, TLS, proxy, or timeout failure. |
| `gateway_verification` | `metadata_only` | Local manifest or focused fixture verification failed. This is implementation evidence, not an external gate. |
| `execution_mode` | `external_gate` | Observed execution mode differs from the manifest without fallback evidence. |

The existing canary currently emits the coarse classes
`credential_or_auth`, `quota_or_rate_limit`, `request_or_route_contract`,
`upstream_or_gateway_server`, `network_or_unknown`, and
`route_proof_missing_or_mismatch`. The runner maps these to the durable failure
classes above. Operators may refine quota, challenge, account, region, or
upstream protocol attribution from the saved response and browser artifacts
without putting response bodies into the evidence JSON.

## Credential And Browser Readiness

Every line record contains six readiness diagnostics:

| Diagnostic | Required when | What `ready` means |
| --- | --- | --- |
| `remoteExecutor` | Browser-backed execution | The configured remote executor (for example, a remote browser executor) is reachable and approved for the line. |
| `localBrowserFallback` | Browser-backed execution | The local browser fallback is installed and usable if an explicit fallback is authorized. |
| `sessionMaterial` | The manifest requires session or browser state | Required session labels resolve to current material outside the evidence file. |
| `credentialRefresh` | Session or bearer material can expire | Refresh/import checks have completed and the credential is not stale. |
| `leaseCooling` | All live provider execution | Credential leasing and cooling state permit a canary attempt. |
| `modelHealth` | All live provider execution | The selected model is currently exposed and healthy enough for the canary. |

Readiness statuses are `ready`, `unavailable`, `not_checked`, or
`not_required`. Offline evidence normally reports `not_checked` for required
external conditions. It never infers readiness from the presence of an
environment variable.

The `fallback` object always records the manifest-declared and observed
execution modes. If a target reports a different `execution_mode`, it must also
provide a non-secret `fallback_reason`. A changed mode without that evidence is
classified as `execution_mode/silent_fallback_blocked`. This prevents remote
executor, local browser, replay, or direct HTTP fallback from being hidden in a
successful provider result.

## Artifacts And Exit Status

Durable evidence JSON is written under `docs/evidence/`. Command logs,
validator reports, canary summaries, headers, and bodies are written under
`target/provider-evidence/` by default and referenced by path.

The runner exits nonzero when local verification fails or when a live record
requires operator attention. Ordinary build and CI flows remain offline, so a
missing provider credential cannot fail the Gateway build unless a caller
explicitly enables live mode.

# Provider quota transport diagnostics

Owner: parallel coordinator. State: regression verified. Started: 2026-09-12 UTC.
Accepted: 2026-09-12 18:21:09 UTC.

The accepted quota probes interpolate rquest transport errors directly into
GatewayError messages. Accio sends its access token in the request URL, and generic
balance paths may also contain private query parameters. Confirm exposure with
real loopback connection/body failures before changing production formatting.

Scope: provider_quota entry test wiring, codex.rs, accio_http.rs,
generic_balance.rs and a fixed loopback contract/fixture. Preserve request URLs,
headers, JSON results, error category/status/code, HTTP-error handling and all
seven original quota tests. The intended fix strips the URL attached to the
transport error before formatting; locked rquest 5.1.0 exposes without_url for
this purpose. No provider request URL or credential delivery is changed.

The frozen loopback contract advances from six passes/two URL-exposure failures
to 8/8. Six statements strip only the attached URL in send/body-read errors for
Codex, Accio and generic balance. Exact source comparison preserves all other
production text, eight function signatures, six structs and 249 neighboring
inputs. Requests, error classification and the frozen test files are unchanged.

Original quota units pass 7/7 before/after. Fresh all-targets, scoped formatter,
checker 19/19, ratchet, source/encoding proof and both Git checks pass. All Cargo
gates are terminal. The six scoped Rust files are at most 169 effective lines.
Strict scans 1,579 files: 31 hard, 30 mandatory and 40 soft; 61 remain above 700.
Global formatter findings remain confined to the two unchanged S06 files.

Fixtures bind random loopback ports, use only synthetic credentials, bound
request reads and cancel owned tasks. The runtime contracts cover Accio/balance;
Codex's fixed endpoint is covered by exact source proof, existing units and the
compiler. No live provider request was made.

HTTP body bounds, provider-body diagnostic redaction, refresh-lock atomicity and
numeric conversion remain separate. S06 ownership, GWP-20260912-01 and the final
freeze/build transfer remain unchanged. This checkpoint does not perform a release
or persistent deployment.

Immutable evidence: target/effective-line-evidence/20260912-provider-quota-diagnostics/scope.json.
Report: [quota transport diagnostics](../../status/2026-09-12-provider-quota-diagnostics.md).

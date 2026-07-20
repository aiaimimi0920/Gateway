# Deployment Cutover Archive

## Status

The Rust `gateway/` cutover is complete.

- Rust `gateway/` is the only active public gateway runtime.
- The old TypeScript gateway has been removed from the repository.
- Default local preview, Compose, K8s ingress, operator calls, and benefit-service management flows now target Rust `gateway`.

## Why This File Still Exists

This file remains as a short archive marker so future runs understand that:

- the cutover plan has already been executed
- old TypeScript gateway rollout and rollback steps are no longer valid
- any future deployment work must extend the Rust gateway, not recreate a second gateway runtime

## Current Deployment Rule

Use the current deployment and architecture baselines instead of historical cutover instructions:

- `docs/20-ai-gateway/AI网关总基线.md`
- `docs/20-ai-gateway/AI网关发布与滚动切流基线.md`
- `docs/10-platform/NeuroLoom平台总基线.md`
- `AGENTS.md`

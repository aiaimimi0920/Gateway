# Folder-sync object storage S3 loopback, 2026-09-18

The provider-account object-storage path now has a local S3-compatible HTTP
regression in addition to the existing local-driver round trip.

## Coverage

- Builds the canonical provider-account key
  `ai-gateway/provider-account/<id>.json`.
- Sends a real AWS SDK `PutObject` request to an axum loopback listener with
  path-style bucket addressing.
- Reads the same object through `GetObject`, including bounded body decoding and
  JSON parsing.
- Deletes the object through `DeleteObject` and records the request route.
- Applies one explicit 30-second S3 network deadline to each production PUT, GET
  (including body consumption), and DELETE operation; the loopback regression uses
  a short deadline to prove a stalled response fails closed.
- Records the request method and path so the test proves the bucket/key route,
  not only a successful response body.

## Verification

- Object-storage library group: 23 passed, 0 failed.
- The S3-compatible provider-account PUT/GET/DELETE round trip passed.
- The stalled response and stalled body network deadline regressions passed.
- The test uses synthetic credentials and a loopback listener only; no remote S3
  deployment or production bucket was contacted.
- Existing local path, junction containment, readiness, pagination, and body-limit
  tests remain green.

Remote S3 success/read, credentials, endpoint policy, and full provider/UI/Docker/
release acceptance remain separate open gates.

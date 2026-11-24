# Amazon Quick (`quick-oauth`) Provider Contract

## Scope

`quick-oauth` integrates Amazon Quick (QuickWork) social OAuth login and its
bidirectional WebSocket conversation stream into Neuro Gateway.

- **Preset / Template ID**: `quick-oauth`
- **Protocol Profile**: `quick_oauth`
- **Auth Mode**: `quick_oauth`
- **Cargo Feature**: `line-quick-oauth`
- **Base URL**: `https://quick.aws.com`
- **Supported Agent Modes**: `smart`, `fast`, `balanced`, `auto`
- **Downstream Surfaces**: `/v1/chat/completions`, `/v1/responses`, `/v1/messages`

## Social OAuth & Durable Token Refresh

1. **Tenant Discovery**:
   - Opens `https://quick.aws.com/sn?qwSurface=desktop%3A<nonce>%3A<port>%3A1.2.4660`.
   - Validates callback `nonce` in constant time and extracts `issuerUrl`,
     optional `tenantUrl`, and commercial AWS IAM Identity Center `idcRegion`
     (default `us-east-1`).
2. **Dynamic Public OIDC Client Registration & PKCE Exchange**:
   - Registers a public client (`quicksight:read_write`) at
     `https://oidc.{idcRegion}.amazonaws.com/client/register`.
   - Completes S256 PKCE authorization at `/authorize` and exchanges the code at
     `/token`. Both automatic loopback callback (`127.0.0.1:<port>`) and manual
     callback URL paste are supported.
   - Fetches account metadata via `/integration/quick-work/user-info`.
3. **Secret Hygiene & Refresh**:
   - `quick_oauth.client_secret`, `api_key`, and `refresh_token` are redacted in
     management reads and preserved on `SecretPatch::Keep`.
   - Durable refresh runs through `TokenRefreshState` + `GrantRecord` before
     `kiro_oauth`'s generic `oidc.*.amazonaws.com` matcher.

## Bidirectional WebSocket Conversation Wire

- Connects to `wss://{ws_host}/streaming/quickwork-chat-bidirectional-stream?surfaceType=QUICK_WORK_DESKTOP_WINDOWS&token={jwt}`.
- Sends `rawInputEvent` frames (`session_connect`, `federate_status`,
  `session_create`, `user_message`) with caller `system`/`developer` prompts,
  an original backend-persona override suffix, optional client tool bridge
  markers (`<client_tool_call>`), and optional `thinking_effort`.
- Decodes binary AWS EventStream frames (`CRC32` verified) for `text_delta`,
  `text_complete`, `turn_usage`, tool events, `message_complete`, and terminal
  events, then sends `session_delete` to clean up temporary conversations.

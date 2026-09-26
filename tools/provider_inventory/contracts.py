"""Canonical inventory contract and repository-owned input paths."""

from __future__ import annotations

import pathlib
import re


GATEWAY_ROOT = pathlib.Path(__file__).resolve().parents[2]
MANIFEST_ROOT = GATEWAY_ROOT / "manifests" / "lines"
CARGO_PATH = GATEWAY_ROOT / "Cargo.toml"
IMPLEMENTATION_LINES_PATH = GATEWAY_ROOT / "src" / "implementation_lines.rs"
PROTOCOL_RESOLUTION_PATH = GATEWAY_ROOT / "src" / "routing" / "protocol_resolution.rs"
PROTOCOL_REGISTRY_PATH = GATEWAY_ROOT / "src" / "protocol" / "registry.rs"
DEFAULT_OUTPUT = GATEWAY_ROOT / "docs" / "provider-inventory.json"
SCHEMA_PATH = GATEWAY_ROOT / "docs" / "provider-inventory.schema.json"

# This is an intentionally checked-in contract.  Manifest discovery is used
# for metadata, but it must never silently redefine the product surface.
CANONICAL_PROVIDER_LINE_IDS = (
    "accio-web-reverse-api",
    "aistudio-official",
    "aistudio-web-reverse",
    "anthropic-messages-official-model-api",
    "aws-bedrock-converse-official-model-api",
    "azure-openai-official-vendor-api",
    "chataibot-web-reverse",
    "chatgpt-codex-oauth-official",
    "chatgpt-official-api",
    "chatgpt-web-reverse",
    "cohere-chat-official-model-api",
    "deepseek-openai-official-model-api",
    "exa-search-official-vendor-api",
    "freebuff-web-reverse-api",
    "gemini-canvas-program",
    "gemini-web-reverse",
    "google-agent-platform-official",
    "grok-web-reverse-api",
    "groq-openai-official-vendor-api",
    "jina-reader-official-vendor-api",
    "jina-search-official-vendor-api",
    "kiro-official-vendor-api",
    "linkup-search-official-vendor-api",
    "longcat-openai-official-model-api",
    "lumalabs-web-reverse-api",
    "mistral-openai-official-model-api",
    "muyuan-openai-aggregator-api",
    "nvidia-openai-official-vendor-api",
    "openrouter-openai-aggregator-api",
    "perplexity-chat-official-vendor-api",
    "perplexity-search-official-vendor-api",
    "producer-web-reverse-api",
    "poe-openai-aggregator-api",
    "qwen-official-api",
    "qwen-web-reverse",
    "suno-web-reverse-api",
    "tavily-search-official-vendor-api",
    "together-openai-aggregator-api",
    "udio-web-reverse-api",
    "websearchapi-search-official-vendor-api",
    "xai-openai-official-vendor-api",
    "xfyun-native-websocket-official-vendor-api",
    "xfyun-openai-official-vendor-api",
    "you-search-official-vendor-api",
)

EVIDENCE_STATES = (
    "compiled",
    "metadata_only",
    "fixture_passed",
    "live_passed",
    "external_gate",
    "credential_missing",
    "known_unsupported",
)

TIMESTAMP_RE = re.compile(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z$")


class InventoryError(RuntimeError):
    """Raised for malformed source metadata or explicit evidence."""

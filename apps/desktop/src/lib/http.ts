import type {
  GatewayApiTestInput,
  GatewayApiTestResult,
  GatewayHealthResponse,
  GatewayHttpProbe,
  GatewayModelListResponse,
  GatewayReadyResponse,
} from "./types";
import type { ZodType } from "zod";
import {
  publicHealthSchema,
  publicModelListSchema,
  publicReadinessSchema,
  unknownObjectSchema,
} from "../api/schemas";

export function gatewayBaseUrl(port?: number | null): string {
  const resolvedPort = Number.isFinite(port ?? NaN) && (port ?? 0) > 0 ? port : 4200;
  return `http://127.0.0.1:${resolvedPort}`;
}

export function gatewayBaseUrlFromProfile(profile: Pick<import("./types").GatewayProfile, "port">): string {
  return gatewayBaseUrl(profile.port);
}

function toErrorMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }
  return String(error);
}

async function decodeResponse(response: Response): Promise<unknown> {
  const text = await response.text();
  if (text.trim().length === 0) {
    return null;
  }

  try {
    return JSON.parse(text) as unknown;
  } catch {
    return text;
  }
}

async function requestJson<TData>(
  url: string,
  schema: ZodType<TData>,
  init?: RequestInit,
): Promise<GatewayHttpProbe<TData>> {
  const startedAt = performance.now();
  try {
    const response = await fetch(url, {
      ...init,
      headers: {
        Accept: "application/json",
        ...(init?.headers ?? {}),
      },
    });
    const payload = await decodeResponse(response);
    if (response.ok) {
      const parsed = schema.safeParse(payload);
      if (!parsed.success) {
        return {
          ok: false,
          status: response.status,
          durationMs: Math.round(performance.now() - startedAt),
          error: `Gateway response schema validation failed: ${parsed.error.issues.map((issue) => issue.message).join("; ")}`,
        };
      }
      return {
        ok: true,
        status: response.status,
        durationMs: Math.round(performance.now() - startedAt),
        data: parsed.data,
      };
    }
    return {
      ok: false,
      status: response.status,
      durationMs: Math.round(performance.now() - startedAt),
      error: response.statusText || "Gateway request failed",
    };
  } catch (error) {
    return {
      ok: false,
      status: 0,
      durationMs: Math.round(performance.now() - startedAt),
      error: toErrorMessage(error),
    };
  }
}

export function fetchGatewayHealth(baseUrl: string): Promise<GatewayHttpProbe<GatewayHealthResponse>> {
  return requestJson<GatewayHealthResponse>(`${baseUrl}/healthz`, publicHealthSchema);
}

export function fetchGatewayReady(baseUrl: string): Promise<GatewayHttpProbe<GatewayReadyResponse>> {
  return requestJson<GatewayReadyResponse>(`${baseUrl}/readyz`, publicReadinessSchema);
}

export function fetchGatewayModels(
  baseUrl: string,
  apiKey?: string,
): Promise<GatewayHttpProbe<GatewayModelListResponse>> {
  const headers: Record<string, string> = {};
  const trimmedApiKey = apiKey?.trim() ?? "";
  if (trimmedApiKey.length > 0) {
    headers.Authorization = `Bearer ${trimmedApiKey}`;
  }
  return requestJson<GatewayModelListResponse>(`${baseUrl}/v1/models`, publicModelListSchema, { headers });
}

export async function runGatewayChatCompletionTest(
  baseUrl: string,
  input: GatewayApiTestInput,
  context: Pick<GatewayApiTestResult, "requestedAt" | "profileName" | "baseUrl" | "model">,
): Promise<GatewayApiTestResult> {
  const endpoint = `${baseUrl}/v1/chat/completions`;
  const headers: Record<string, string> = {
    "Content-Type": "application/json",
  };
  if (input.apiKey.trim().length > 0) {
    headers.Authorization = `Bearer ${input.apiKey.trim()}`;
  }

  const probe = await requestJson<Record<string, unknown>>(endpoint, unknownObjectSchema, {
    method: "POST",
    headers,
    body: JSON.stringify({
      model: input.model.trim(),
      messages: [{ role: "user", content: input.message }],
      stream: false,
    }),
  });

  return {
    ...probe,
    endpoint,
    ...context,
  };
}

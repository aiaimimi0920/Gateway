import type {
  GatewayApiTestInput,
  GatewayApiTestResult,
  GatewayHealthResponse,
  GatewayHttpProbe,
  GatewayModelListResponse,
  GatewayReadyResponse,
} from "./types";

export function gatewayBaseUrl(port?: number | null): string {
  const resolvedPort = Number.isFinite(port ?? NaN) && (port ?? 0) > 0 ? port : 4200;
  return `http://127.0.0.1:${resolvedPort}`;
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
    const payload = (await decodeResponse(response)) as TData;
    return {
      ok: response.ok,
      status: response.status,
      durationMs: Math.round(performance.now() - startedAt),
      data: payload,
      error: response.ok ? undefined : response.statusText || "Gateway request failed",
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
  return requestJson<GatewayHealthResponse>(`${baseUrl}/healthz`);
}

export function fetchGatewayReady(baseUrl: string): Promise<GatewayHttpProbe<GatewayReadyResponse>> {
  return requestJson<GatewayReadyResponse>(`${baseUrl}/readyz`);
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
  return requestJson<GatewayModelListResponse>(`${baseUrl}/v1/models`, { headers });
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

  const probe = await requestJson<unknown>(endpoint, {
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

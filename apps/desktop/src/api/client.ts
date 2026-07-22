import { z, type ZodType } from "zod";
import type { GatewayHostAdapter } from "../platform/types";
import { GatewayApiError, GatewayResponseValidationError } from "./errors";

const gatewayErrorEnvelopeSchema = z.object({
  error: z.object({
    message: z.string().optional(),
    type: z.string().optional(),
    code: z.string().nullable().optional(),
  }),
});

export type GatewayApiRequestOptions = Omit<RequestInit, "body" | "headers"> & {
  body?: unknown;
  headers?: HeadersInit;
  managementToken?: string | null;
  secretGrant?: string | null;
};

export type GatewayApiClient = {
  request<T>(path: string, schema: ZodType<T>, options?: GatewayApiRequestOptions): Promise<T>;
};

export type CreateGatewayApiClientOptions = {
  host: GatewayHostAdapter;
  fetchImplementation?: typeof fetch;
  onAuthenticationFailure?: (status: 401 | 403) => void;
};

function buildApiUrl(origin: string, path: string): string {
  if (!path.startsWith("/") || path.startsWith("//")) {
    throw new Error("Gateway API paths must be absolute same-origin paths.");
  }
  const url = new URL(path, `${origin}/`);
  if (url.origin !== origin) {
    throw new Error("Gateway API requests cannot leave the selected host origin.");
  }
  return url.toString();
}

async function decodeJson(response: Response, path: string): Promise<unknown> {
  const text = await response.text();
  if (text.trim().length === 0) {
    return null;
  }
  try {
    return JSON.parse(text) as unknown;
  } catch {
    throw new GatewayResponseValidationError(path, ["Response body is not valid JSON."]);
  }
}

async function decodeGatewayError(response: Response): Promise<unknown> {
  const text = await response.text();
  if (text.trim().length === 0) {
    return null;
  }
  try {
    return JSON.parse(text) as unknown;
  } catch {
    return null;
  }
}

function parseGatewayError(payload: unknown, status: number): GatewayApiError {
  const parsed = gatewayErrorEnvelopeSchema.safeParse(payload);
  if (!parsed.success) {
    return new GatewayApiError(`Gateway request failed with HTTP ${status}.`, status);
  }
  return new GatewayApiError(
    parsed.data.error.message ?? `Gateway request failed with HTTP ${status}.`,
    status,
    parsed.data.error.code ?? undefined,
    parsed.data.error.type,
  );
}

export function createGatewayApiClient({
  host,
  fetchImplementation = fetch,
  onAuthenticationFailure,
}: CreateGatewayApiClientOptions): GatewayApiClient {
  return {
    async request<T>(
      path: string,
      schema: ZodType<T>,
      options: GatewayApiRequestOptions = {},
    ): Promise<T> {
      const {
        body,
        headers: suppliedHeaders,
        managementToken,
        secretGrant,
        ...requestInit
      } = options;
      const headers = new Headers(suppliedHeaders);
      headers.set("Accept", "application/json");
      if (body !== undefined) {
        headers.set("Content-Type", "application/json");
      }
      if (managementToken) {
        headers.set("x-management-token", managementToken);
      }
      if (secretGrant) {
        headers.set("x-secret-grant", secretGrant);
      }

      const response = await fetchImplementation(buildApiUrl(host.apiOrigin, path), {
        ...requestInit,
        body: body === undefined ? undefined : JSON.stringify(body),
        cache: "no-store",
        credentials: "omit",
        headers,
        referrerPolicy: "no-referrer",
      });
      if (!response.ok) {
        if (response.status === 401 || response.status === 403) {
          onAuthenticationFailure?.(response.status);
        }
        const payload = await decodeGatewayError(response);
        throw parseGatewayError(payload, response.status);
      }

      const payload = await decodeJson(response, path);
      const parsed = schema.safeParse(payload);
      if (!parsed.success) {
        throw new GatewayResponseValidationError(
          path,
          parsed.error.issues.map((issue) => issue.message),
        );
      }
      return parsed.data;
    },
  };
}

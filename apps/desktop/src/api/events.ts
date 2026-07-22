import {
  fetchEventSource as defaultFetchEventSource,
  type EventSourceMessage,
  type FetchEventSourceInit,
} from "@microsoft/fetch-event-source";
import type { GatewayHostAdapter } from "../platform/types";
import type { ConsoleEvent } from "./contracts";
import {
  GatewayApiError,
  GatewayResponseValidationError,
} from "./errors";
import { consoleEventSchema } from "./schemas";

type FetchEventSource = (input: string, init: FetchEventSourceInit) => Promise<void>;

const RETRYABLE_STREAM_STATUSES = new Set([408, 423, 424, 425, 429]);

export type ConsoleEventSubscription = {
  host: GatewayHostAdapter;
  managementToken: string;
  onEvent: (event: ConsoleEvent) => void;
  signal?: AbortSignal;
  onError?: (error: unknown) => void;
  onAuthenticationFailure?: (status: 401 | 403) => void;
};

function isFatalStreamError(error: unknown): boolean {
  return (
    error instanceof GatewayResponseValidationError ||
    (error instanceof GatewayApiError &&
      error.status < 500 &&
      !RETRYABLE_STREAM_STATUSES.has(error.status))
  );
}

function parseEventMessage(message: EventSourceMessage): ConsoleEvent | null {
  if (message.data.trim().length === 0) {
    return null;
  }
  let payload: unknown;
  try {
    payload = JSON.parse(message.data) as unknown;
  } catch {
    throw new GatewayResponseValidationError("/events/stream", ["SSE data is not valid JSON."]);
  }
  const parsed = consoleEventSchema.safeParse(payload);
  if (!parsed.success) {
    throw new GatewayResponseValidationError(
      "/events/stream",
      parsed.error.issues.map((issue) => issue.message),
    );
  }
  return parsed.data;
}

export function subscribeToConsoleEvents(
  subscription: ConsoleEventSubscription,
  fetchEventSource: FetchEventSource = defaultFetchEventSource,
): Promise<void> {
  const url = new URL(
    "/v1/internal/gateway/console/events/stream",
    `${subscription.host.apiOrigin}/`,
  ).toString();
  return fetchEventSource(url, {
    method: "GET",
    credentials: "omit",
    headers: {
      Accept: "text/event-stream",
      "x-management-token": subscription.managementToken,
    },
    openWhenHidden: true,
    signal: subscription.signal,
    async onopen(response) {
      if (response.ok) {
        const contentType = response.headers.get("content-type") ?? "";
        if (contentType.toLowerCase().startsWith("text/event-stream")) {
          return;
        }
        throw new GatewayResponseValidationError("/events/stream", [
          "Event stream response is not text/event-stream.",
        ]);
      }
      const status = response.status;
      if (status === 401 || status === 403) {
        subscription.onAuthenticationFailure?.(status);
      }
      throw new GatewayApiError(
        `Gateway event stream failed with HTTP ${status}.`,
        status,
        status === 401 || status === 403 ? "console_management_token_invalid" : undefined,
      );
    },
    onmessage(message) {
      const event = parseEventMessage(message);
      if (event) {
        subscription.onEvent(event);
      }
    },
    onclose() {
      throw new Error("Gateway event stream closed; retrying.");
    },
    onerror(error) {
      subscription.onError?.(error);
      if (isFatalStreamError(error)) {
        throw error;
      }
    },
  });
}

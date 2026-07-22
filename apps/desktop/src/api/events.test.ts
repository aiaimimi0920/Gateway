import type { FetchEventSourceInit } from "@microsoft/fetch-event-source";
import { describe, expect, it, vi } from "vitest";
import { createTauriHost } from "../platform/tauriHost";
import {
  clearManagementSession,
  readManagementSessionToken,
  writeManagementSessionToken,
} from "../session/storage";
import { GatewayResponseValidationError } from "./errors";
import { subscribeToConsoleEvents } from "./events";

describe("console event stream", () => {
  it("uses fetch-SSE with the management header and no URL credential", async () => {
    let observedUrl = "";
    let observedInit: FetchEventSourceInit | undefined;
    const fetchEventSource = vi.fn(async (url: string, init: FetchEventSourceInit) => {
      observedUrl = url;
      observedInit = init;
    });

    await subscribeToConsoleEvents(
      {
        host: createTauriHost("http://127.0.0.1:44200"),
        managementToken: "management-secret",
        onEvent: vi.fn(),
      },
      fetchEventSource,
    );

    expect(fetchEventSource).toHaveBeenCalledOnce();
    expect(observedUrl).toBe(
      "http://127.0.0.1:44200/v1/internal/gateway/console/events/stream",
    );
    expect(observedUrl).not.toContain("management-secret");
    expect(observedInit?.headers).toMatchObject({
      Accept: "text/event-stream",
      "x-management-token": "management-secret",
    });
  });

  it("treats a clean event-stream EOF as retryable", async () => {
    let observedInit: FetchEventSourceInit | undefined;
    const fetchEventSource = vi.fn(async (_url: string, init: FetchEventSourceInit) => {
      observedInit = init;
    });

    await subscribeToConsoleEvents(
      {
        host: createTauriHost("http://127.0.0.1:44200"),
        managementToken: "management-secret",
        onEvent: vi.fn(),
      },
      fetchEventSource,
    );

    expect(() => observedInit?.onclose?.()).toThrow(/closed.*retry/i);
  });

  it.each([401, 403])("treats HTTP %s as fatal and clears the session", async (status) => {
    writeManagementSessionToken("management-secret");
    const onAuthenticationFailure = vi.fn(() => clearManagementSession());
    const fetchEventSource = vi.fn(async (_url: string, init: FetchEventSourceInit) => {
      try {
        await init.onopen?.(
          new Response(null, {
            status,
            headers: { "content-type": "application/json" },
          }),
        );
      } catch (error) {
        init.onerror?.(error);
      }
    });

    const stream = subscribeToConsoleEvents(
      {
        host: createTauriHost("http://127.0.0.1:44200"),
        managementToken: "management-secret",
        onAuthenticationFailure,
        onEvent: vi.fn(),
      },
      fetchEventSource,
    );

    await expect(stream).rejects.toMatchObject({ status });
    expect(onAuthenticationFailure).toHaveBeenCalledWith(status);
    expect(readManagementSessionToken()).toBeNull();
    expect(fetchEventSource).toHaveBeenCalledOnce();
  });

  it.each([408, 423, 424, 425, 429])("treats temporary HTTP %s as retryable", async (status) => {
    const onError = vi.fn();
    const fetchEventSource = vi.fn(async (_url: string, init: FetchEventSourceInit) => {
      try {
        await init.onopen?.(
          new Response(null, {
            status,
            headers: { "content-type": "application/json" },
          }),
        );
      } catch (error) {
        init.onerror?.(error);
      }
    });

    await expect(
      subscribeToConsoleEvents(
        {
          host: createTauriHost("http://127.0.0.1:44200"),
          managementToken: "management-secret",
          onError,
          onEvent: vi.fn(),
        },
        fetchEventSource,
      ),
    ).resolves.toBeUndefined();
    expect(onError).toHaveBeenCalledOnce();
  });

  it("treats malformed event payloads as fatal instead of reconnecting", async () => {
    const fetchEventSource = vi.fn(async (_url: string, init: FetchEventSourceInit) => {
      try {
        init.onmessage?.({ id: "event-1", event: "config", data: "{\"id\":\"event-1\"}" });
      } catch (error) {
        init.onerror?.(error);
      }
    });

    const stream = subscribeToConsoleEvents(
      {
        host: createTauriHost("http://127.0.0.1:44200"),
        managementToken: "management-secret",
        onEvent: vi.fn(),
      },
      fetchEventSource,
    );

    await expect(stream).rejects.toBeInstanceOf(GatewayResponseValidationError);
    expect(fetchEventSource).toHaveBeenCalledOnce();
  });
});

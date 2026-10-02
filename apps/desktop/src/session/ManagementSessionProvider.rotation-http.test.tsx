import { act, renderHook, waitFor } from "@testing-library/react";
import { http, HttpResponse } from "msw";
import { beforeEach, describe, expect, it } from "vitest";
import { HostProvider } from "../platform/HostProvider";
import { createBrowserHost } from "../platform/browserHost";
import { server } from "../test/server";
import { ManagementSessionProvider } from "./ManagementSessionProvider";
import { authenticatedSession } from "./managementSessionTestFixtures";
import { readManagementSessionToken, writeManagementSessionToken } from "./storage";
import { useManagementSession } from "./useManagementSession";

describe("management rotation through the real API client", () => {
  beforeEach(() => { sessionStorage.clear(); localStorage.clear(); });

  it.each([true, false])("handles a lost response with committed=%s without invalidating an unverified current token", async committed => {
    let acceptedToken = "synthetic-old";
    let rotations = 0;
    const candidates: string[] = [];
    const root = `${location.origin}/v1/internal/gateway/console`;
    server.use(
      http.get(`${root}/bootstrap/status`, () => HttpResponse.json({
        needsBootstrap: false, managementConfigured: true, environmentOverride: false,
      })),
      http.post(`${root}/session/verify`, ({ request }) => {
        const token = request.headers.get("x-management-token") ?? "";
        candidates.push(token);
        expect(request.url).not.toContain(token);
        return token === acceptedToken ? HttpResponse.json(authenticatedSession)
          : HttpResponse.json({ error: { message: "Rejected" } }, { status: 401 });
      }),
      http.post(`${root}/session/rotate`, async ({ request }) => {
        rotations += 1;
        expect(request.headers.get("x-management-token")).toBe("synthetic-old");
        expect(await request.json()).toEqual({ newToken: "synthetic-new" });
        if (committed) acceptedToken = "synthetic-new";
        return HttpResponse.error();
      }),
    );
    writeManagementSessionToken(acceptedToken);
    const { result } = renderHook(useManagementSession, {
      wrapper: ({ children }) => <HostProvider adapter={createBrowserHost()}>
        <ManagementSessionProvider>{children}</ManagementSessionProvider>
      </HostProvider>,
    });
    await waitFor(() => expect(result.current.phase).toBe("authenticated"));
    let failure: unknown;
    await act(async () => { await result.current.rotate("synthetic-new").catch(error => { failure = error; }); });
    expect(rotations).toBe(1);
    expect(candidates).toEqual(["synthetic-old", "synthetic-new"]);
    expect(result.current.phase).toBe("authenticated");
    expect(result.current.managementToken).toBe(acceptedToken);
    expect(readManagementSessionToken()).toBe(acceptedToken);
    expect(Boolean(failure)).toBe(!committed);
    expect(localStorage.length).toBe(0);
  });
});

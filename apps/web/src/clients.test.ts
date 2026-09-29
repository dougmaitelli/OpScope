import { describe, expect, it, vi } from "vitest";
import { HttpClient } from "./clients.ts";

describe("HTTP session expiration", () => {
  it("does not expire the session for provider credential failures", async () => {
    const dispatch = vi.spyOn(globalThis, "dispatchEvent");
    const client = new HttpClient(async () =>
      Response.json({ message: "Invalid provider credential" }, { status: 401 }),
    );

    await expect(
      client.connectSource({
        connectionId: null,
        sourceId: "github",
        configuration: {},
        credential: "invalid",
      }),
    ).rejects.toThrow("Invalid provider credential");
    await expect(client.listWorkflows()).rejects.toThrow("Invalid provider credential");
    expect(dispatch).not.toHaveBeenCalled();
  });

  it("dispatches session expiration for an explicit middleware 401", async () => {
    const dispatch = vi.spyOn(globalThis, "dispatchEvent");
    const client = new HttpClient(
      async () =>
        new Response(null, {
          status: 401,
          headers: { "x-opsscope-session-error": "unauthenticated" },
        }),
    );

    await expect(client.listWorkflows()).rejects.toThrow("401");
    expect(dispatch).toHaveBeenCalledExactlyOnceWith(
      expect.objectContaining({ type: "opsscope:unauthorized" }),
    );
  });

  it.each([
    [403, "unauthenticated"],
    [401, "unknown"],
  ])("does not expire the session for status %s and marker %s", async (status, marker) => {
    const dispatch = vi.spyOn(globalThis, "dispatchEvent");
    const client = new HttpClient(
      async () =>
        new Response(null, {
          status,
          headers: { "x-opsscope-session-error": marker },
        }),
    );

    await expect(client.listWorkflows()).rejects.toThrow(String(status));
    expect(dispatch).not.toHaveBeenCalled();
  });
});

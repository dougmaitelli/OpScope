import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { HttpClient } from "../clients.ts";
import { setHttpCsrfToken } from "../api/application-client.tsx";
import { WebAuthentication, useWebAuthentication } from "./WebAuthentication.tsx";

const session = {
  enabled: true,
  authenticated: true,
  user: { subject: "user-1", email: "user@example.com" },
  csrfToken: "test-csrf",
};

afterEach(() => {
  setHttpCsrfToken(null);
  window.history.replaceState(null, "", "/");
});

function Account() {
  const { user, logout } = useWebAuthentication();
  return (
    <>
      <p>{user?.email}</p>
      <button onClick={() => void logout()}>Sign out</button>
    </>
  );
}

describe("WebAuthentication", () => {
  it("shows a loading state until the session is verified", async () => {
    let resolveSession!: (response: Response) => void;
    vi.stubGlobal(
      "fetch",
      vi.fn(
        () =>
          new Promise<Response>((resolve) => {
            resolveSession = resolve;
          }),
      ),
    );
    render(
      <WebAuthentication>
        <p>Workflows</p>
      </WebAuthentication>,
    );
    expect(screen.getByRole("heading", { name: "Checking session" })).toBeInTheDocument();
    expect(screen.queryByText("Workflows")).not.toBeInTheDocument();
    await act(async () => resolveSession(Response.json(session)));
    expect(await screen.findByText("Workflows")).toBeInTheDocument();
  });

  it("allows the app when OIDC is disabled, even after an unauthorized event", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () =>
        Response.json({ enabled: false, authenticated: false, user: null, csrfToken: null }),
      ),
    );
    render(
      <WebAuthentication>
        <p>Workflows</p>
      </WebAuthentication>,
    );
    expect(await screen.findByText("Workflows")).toBeInTheDocument();
    act(() => {
      globalThis.dispatchEvent(new Event("opscope:unauthorized"));
    });
    expect(screen.getByText("Workflows")).toBeInTheDocument();
  });

  it("preserves the requested route in the sign-in link", async () => {
    window.history.replaceState(null, "", "/issues?state=open");
    vi.stubGlobal(
      "fetch",
      vi.fn(async () =>
        Response.json({ ...session, authenticated: false, user: null, csrfToken: null }),
      ),
    );
    render(
      <WebAuthentication>
        <p>Workflows</p>
      </WebAuthentication>,
    );
    expect(
      await screen.findByRole("link", { name: "Sign in with OpenID Connect" }),
    ).toHaveAttribute("href", "/api/auth/login?returnTo=%2Fissues%3Fstate%3Dopen");
    expect(screen.queryByText("Workflows")).not.toBeInTheDocument();
  });

  it("keeps the app mounted for provider failures but signs out on session expiration", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => Response.json(session)),
    );
    render(
      <WebAuthentication>
        <p>Workflows</p>
      </WebAuthentication>,
    );
    await screen.findByText("Workflows");
    const provider = new HttpClient(async () =>
      Response.json({ message: "Invalid provider credential" }, { status: 401 }),
    );
    await act(async () => {
      await expect(provider.listWorkflows()).rejects.toThrow("Invalid provider credential");
    });
    expect(screen.getByText("Workflows")).toBeInTheDocument();
    const expired = new HttpClient(
      async () =>
        new Response(null, {
          status: 401,
          headers: { "x-opscope-session-error": "unauthenticated" },
        }),
    );
    await act(async () => {
      await expect(expired.listWorkflows()).rejects.toThrow("401");
    });
    expect(await screen.findByRole("heading", { name: "Sign in required" })).toBeInTheDocument();
    expect(screen.queryByText("Workflows")).not.toBeInTheDocument();
  });

  it("retries a failed session lookup", async () => {
    const user = userEvent.setup();
    const fetch = vi
      .fn<typeof globalThis.fetch>()
      .mockRejectedValueOnce(new Error("offline"))
      .mockResolvedValueOnce(Response.json(session));
    vi.stubGlobal("fetch", fetch);
    render(
      <WebAuthentication>
        <p>Workflows</p>
      </WebAuthentication>,
    );
    expect(
      await screen.findByRole("heading", { name: "Unable to verify your session" }),
    ).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Try again" }));
    expect(await screen.findByText("Workflows")).toBeInTheDocument();
    expect(fetch).toHaveBeenCalledTimes(2);
  });

  it("sends the CSRF token on logout and returns to the sign-in screen", async () => {
    const user = userEvent.setup();
    const fetch = vi
      .fn<typeof globalThis.fetch>()
      .mockResolvedValueOnce(Response.json(session))
      .mockResolvedValueOnce(new Response(null, { status: 204 }));
    vi.stubGlobal("fetch", fetch);
    render(
      <WebAuthentication>
        <Account />
      </WebAuthentication>,
    );
    expect(await screen.findByText("user@example.com")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Sign out" }));
    expect(fetch).toHaveBeenLastCalledWith("/api/auth/logout", {
      method: "POST",
      headers: { "X-CSRF-Token": "test-csrf" },
      credentials: "same-origin",
    });
    expect(await screen.findByRole("heading", { name: "Sign in required" })).toBeInTheDocument();
  });
});

import {
  createContext,
  type ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
} from "react";
import {
  isDesktopRuntime,
  setHttpCsrfToken,
} from "../api/application-client.tsx";
import "./WebAuthentication.css";

const SESSION_PATH = "/api/auth/session";
const LOGIN_PATH = "/api/auth/login";
const LOGOUT_PATH = "/api/auth/logout";

interface AuthenticatedUser {
  subject: string;
  email: string | null;
}

interface AuthenticationStatus {
  enabled: boolean;
  authenticated: boolean;
  user: AuthenticatedUser | null;
  csrfToken: string | null;
}

interface AuthenticationContextValue {
  user: AuthenticatedUser | null;
  webAuthentication: boolean;
  logout(): Promise<void>;
}

const AuthenticationContext = createContext<AuthenticationContextValue | null>(null);

export function WebAuthentication({ children }: { children: ReactNode }) {
  const [status, setStatus] = useState<AuthenticationStatus | null>(
    isDesktopRuntime
      ? { enabled: false, authenticated: false, user: null, csrfToken: null }
      : null,
  );
  const [error, setError] = useState<string | null>(null);

  const loadSession = useCallback(async () => {
    if (isDesktopRuntime) return;
    setError(null);
    try {
      const response = await fetch(SESSION_PATH, {
        headers: { Accept: "application/json" },
        credentials: "same-origin",
      });
      if (!response.ok) throw new Error(`Session request failed with ${response.status}`);
      const next = (await response.json()) as AuthenticationStatus;
      setHttpCsrfToken(next.enabled && next.authenticated ? next.csrfToken : null);
      setStatus(next);
    } catch {
      setHttpCsrfToken(null);
      setError("The authentication service is unavailable.");
    }
  }, []);

  useEffect(() => {
    void loadSession();
    const unauthorized = () => {
      setHttpCsrfToken(null);
      setStatus((current) => current?.enabled === false
        ? current
        : { enabled: true, authenticated: false, user: null, csrfToken: null });
    };
    globalThis.addEventListener("ciwatcher:unauthorized", unauthorized);
    return () => globalThis.removeEventListener("ciwatcher:unauthorized", unauthorized);
  }, [loadSession]);

  const logout = useCallback(async () => {
    if (isDesktopRuntime || !status?.enabled || !status.csrfToken) return;
    const response = await fetch(LOGOUT_PATH, {
      method: "POST",
      headers: { "X-CSRF-Token": status.csrfToken },
      credentials: "same-origin",
    });
    if (!response.ok && response.status !== 401) return;
    setHttpCsrfToken(null);
    setStatus({ enabled: true, authenticated: false, user: null, csrfToken: null });
  }, [status]);

  const context = useMemo<AuthenticationContextValue>(
    () => ({
      user: status?.user ?? null,
      webAuthentication: !isDesktopRuntime && (status?.enabled ?? false),
      logout,
    }),
    [logout, status?.user],
  );

  if (error) {
    return (
      <AuthenticationScreen>
        <h1>Unable to verify your session</h1>
        <p className="auth-error">{error}</p>
        <button className="auth-action" type="button" onClick={() => void loadSession()}>
          Try again
        </button>
      </AuthenticationScreen>
    );
  }

  if (!status) {
    return (
      <AuthenticationScreen>
        <h1>Checking session</h1>
        <p>Contacting the CI Watcher server.</p>
      </AuthenticationScreen>
    );
  }

  if (status.enabled && !status.authenticated) {
    const returnTo = `${window.location.pathname}${window.location.search}`;
    const loginUrl = `${LOGIN_PATH}?returnTo=${encodeURIComponent(returnTo)}`;
    return (
      <AuthenticationScreen>
        <h1>Sign in required</h1>
        <p>Authenticate with the identity provider configured by your administrator.</p>
        <a className="auth-action" href={loginUrl}>Sign in with OpenID Connect</a>
      </AuthenticationScreen>
    );
  }

  return (
    <AuthenticationContext.Provider value={context}>
      {children}
    </AuthenticationContext.Provider>
  );
}

export function useWebAuthentication(): AuthenticationContextValue {
  const value = useContext(AuthenticationContext);
  if (!value) throw new Error("web authentication provider is missing");
  return value;
}

function AuthenticationScreen({ children }: { children: ReactNode }) {
  return (
    <main className="auth-screen">
      <section className="auth-panel" aria-live="polite">
        <div className="auth-brand">
          <span className="auth-mark" aria-hidden="true">CI</span>
          <span>CI Watcher</span>
        </div>
        {children}
      </section>
    </main>
  );
}

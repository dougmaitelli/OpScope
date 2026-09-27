import { useEffect } from "react";
import { NavLink, Outlet, useLocation } from "react-router-dom";
import { applicationVersion } from "../generated/contracts.ts";
import { useWebAuthentication } from "../auth/WebAuthentication.tsx";
import "./AppShell.css";

const pageTitles: Record<string, string> = {
  "/": "Overview",
  "/activity": "Activity",
  "/repositories": "Repositories",
  "/connections": "Connections",
  "/settings": "Settings",
};

export function AppShell() {
  const location = useLocation();
  const authentication = useWebAuthentication();

  useEffect(() => {
    document.title = `${pageTitles[location.pathname] ?? "Overview"} · OpsScope`;
  }, [location.pathname]);

  const navClass = ({ isActive }: { isActive: boolean }) =>
    `nav-item${isActive ? " nav-item-active" : ""}`;

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand">
          <img className="brand-mark" src="/opsscope-logo.png" alt="" />
          <span>OpsScope</span>
        </div>

        <nav aria-label="Primary navigation">
          <NavLink className={navClass} to="/" end>
            <span className="nav-symbol" aria-hidden="true">
              O
            </span>
            Overview
          </NavLink>
          <NavLink className={navClass} to="/activity">
            <span className="nav-symbol" aria-hidden="true">
              A
            </span>
            Activity
          </NavLink>
          <NavLink className={navClass} to="/repositories">
            <span className="nav-symbol" aria-hidden="true">
              R
            </span>
            Repositories
          </NavLink>
          <NavLink className={navClass} to="/connections">
            <span className="nav-symbol" aria-hidden="true">
              C
            </span>
            Connections
          </NavLink>
          <NavLink className={navClass} to="/settings">
            <span className="nav-symbol" aria-hidden="true">
              S
            </span>
            Settings
          </NavLink>
        </nav>

        <div className="sidebar-footer">
          {authentication.webAuthentication ? (
            <div className="sidebar-session">
              <span title={authentication.user?.subject}>
                {authentication.user?.email ?? authentication.user?.subject}
              </span>
              <button type="button" onClick={() => void authentication.logout()}>
                Sign out
              </button>
            </div>
          ) : null}
          <span>Version {applicationVersion}</span>
        </div>
      </aside>

      <main className="workspace">
        <Outlet />
      </main>
    </div>
  );
}

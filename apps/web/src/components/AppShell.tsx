import { useEffect, useState } from "react";
import { NavLink, Outlet, useLocation } from "react-router-dom";
import { useApplicationClient } from "../api/application-client.tsx";
import { applicationVersion, type UpdateStatusResponse } from "../generated/contracts.ts";
import { useWebAuthentication } from "../auth/WebAuthentication.tsx";
import "./AppShell.css";

const pageTitles: Record<string, string> = {
  "/": "Overview",
  "/activity": "Activity",
  "/pull-requests": "Pull requests",
  "/issues": "Issues",
  "/repositories": "Repositories",
  "/connections": "Connections",
  "/settings": "Settings",
};

export function AppShell() {
  const location = useLocation();
  const authentication = useWebAuthentication();
  const client = useApplicationClient();
  const [updateStatus, setUpdateStatus] = useState<UpdateStatusResponse | null>(null);

  useEffect(() => {
    document.title = `${pageTitles[location.pathname] ?? "Overview"} · OpsScope`;
  }, [location.pathname]);

  useEffect(() => {
    let active = true;
    void client
      .updateStatus()
      .then((status) => {
        if (active) setUpdateStatus(status);
      })
      .catch(() => undefined);
    return () => {
      active = false;
    };
  }, [client]);

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
          <NavLink className={navClass} to="/pull-requests">
            <span className="nav-symbol" aria-hidden="true">
              P
            </span>
            Pull requests
          </NavLink>
          <NavLink className={navClass} to="/issues">
            <span className="nav-symbol" aria-hidden="true">
              I
            </span>
            Issues
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
          <div
            className={`sidebar-version${updateStatus?.updateAvailable ? " sidebar-version-update" : ""}`}
          >
            {updateStatus?.updateAvailable ? (
              <a
                className="sidebar-update"
                href={updateStatus.releaseUrl}
                target="_blank"
                rel="noreferrer"
              >
                Update available · v{updateStatus.latestVersion}
              </a>
            ) : null}
            <span>Version {applicationVersion}</span>
          </div>
        </div>
      </aside>

      <main className="workspace">
        <Outlet />
      </main>
    </div>
  );
}

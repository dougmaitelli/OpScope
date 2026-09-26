import { useEffect, useState } from "react";
import { NavLink, Outlet, useLocation } from "react-router-dom";
import { useApplicationClient } from "../api/application-client.tsx";
import "./AppShell.css";

const pageTitles: Record<string, string> = {
  "/": "Overview",
  "/repositories": "Repositories",
  "/connections": "Connections",
};

export function AppShell() {
  const client = useApplicationClient();
  const location = useLocation();
  const [serviceState, setServiceState] = useState("Connecting to core");
  const [serviceOnline, setServiceOnline] = useState<boolean | null>(null);

  useEffect(() => {
    document.title = `${pageTitles[location.pathname] ?? "Overview"} · CI Watcher`;
  }, [location.pathname]);

  useEffect(() => {
    let current = true;
    void client.health().then(
      (health) => {
        if (current) {
          setServiceOnline(true);
          setServiceState(`Core online · contract v${health.contractVersion}`);
        }
      },
      () => {
        if (current) {
          setServiceOnline(false);
          setServiceState("Core unavailable");
        }
      },
    );
    return () => {
      current = false;
    };
  }, [client]);

  const navClass = ({ isActive }: { isActive: boolean }) =>
    `nav-item${isActive ? " nav-item-active" : ""}`;

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand">
          <span className="brand-mark" aria-hidden="true">CI</span>
          <span>CI Watcher</span>
        </div>

        <nav aria-label="Primary navigation">
          <NavLink className={navClass} to="/" end>
            <span className="nav-symbol" aria-hidden="true">O</span>
            Overview
          </NavLink>
          <button className="nav-item" type="button" disabled>
            <span className="nav-symbol" aria-hidden="true">A</span>
            Activity
          </button>
          <NavLink className={navClass} to="/repositories">
            <span className="nav-symbol" aria-hidden="true">R</span>
            Repositories
          </NavLink>
          <NavLink className={navClass} to="/connections">
            <span className="nav-symbol" aria-hidden="true">C</span>
            Connections
          </NavLink>
        </nav>

        <div className="sidebar-footer">
          <span
            className={`service-dot${serviceOnline === true ? " service-dot-online" : ""}${serviceOnline === false ? " service-dot-error" : ""}`}
            aria-hidden="true"
          />
          <span>{serviceState}</span>
        </div>
      </aside>

      <main className="workspace">
        <Outlet />
      </main>
    </div>
  );
}

import { useEffect } from "react";
import { NavLink, Outlet, useLocation } from "react-router-dom";
import { applicationVersion } from "../generated/contracts.ts";
import "./AppShell.css";

const pageTitles: Record<string, string> = {
  "/": "Overview",
  "/repositories": "Repositories",
  "/connections": "Connections",
  "/settings": "Settings",
};

export function AppShell() {
  const location = useLocation();

  useEffect(() => {
    document.title = `${pageTitles[location.pathname] ?? "Overview"} · CI Watcher`;
  }, [location.pathname]);

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
          <NavLink className={navClass} to="/settings">
            <span className="nav-symbol" aria-hidden="true">S</span>
            Settings
          </NavLink>
        </nav>

        <div className="sidebar-footer">
          Version {applicationVersion}
        </div>
      </aside>

      <main className="workspace">
        <Outlet />
      </main>
    </div>
  );
}

import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { useApplicationClient } from "../api/application-client.tsx";
import "./PersonalScopeNotice.css";

export function PersonalScopeNotice() {
  const client = useApplicationClient();
  const [enabled, setEnabled] = useState(false);
  useEffect(() => {
    let active = true;
    void client
      .getSettings()
      .then((settings) => {
        if (active) setEnabled(settings.onlyMyWork);
      })
      .catch(() => undefined);
    return () => {
      active = false;
    };
  }, [client]);
  if (!enabled) return null;
  return (
    <aside className="personal-scope-notice" aria-label="Personal monitoring scope">
      <strong>My work</strong>
      <span>
        Showing items related to each connection’s token owner. Workflow rows show the latest
        matching run and matching history. Unverified items are hidden. Notifications use this scope
        too.
      </span>
      <Link to="/settings">Settings</Link>
    </aside>
  );
}

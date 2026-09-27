import { Navigate, Route, Routes } from "react-router-dom";
import { AppShell } from "./components/AppShell.tsx";
import { ActivityPage } from "./pages/activity/ActivityPage.tsx";
import { ConnectionsPage } from "./pages/connections/ConnectionsPage.tsx";
import { OverviewPage } from "./pages/overview/OverviewPage.tsx";
import { RepositoriesPage } from "./pages/repositories/RepositoriesPage.tsx";
import { SettingsPage } from "./pages/settings/SettingsPage.tsx";

export function App() {
  return (
    <Routes>
      <Route element={<AppShell />}>
        <Route index element={<OverviewPage />} />
        <Route path="activity" element={<ActivityPage />} />
        <Route path="repositories" element={<RepositoriesPage />} />
        <Route path="connections" element={<ConnectionsPage />} />
        <Route path="settings" element={<SettingsPage />} />
        <Route path="*" element={<Navigate to="/" replace />} />
      </Route>
    </Routes>
  );
}

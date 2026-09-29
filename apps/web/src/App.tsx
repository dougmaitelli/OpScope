import { Navigate, Route, Routes } from "react-router-dom";
import { AppShell } from "./components/AppShell.tsx";
import { ActivityPage } from "./pages/activity/ActivityPage.tsx";
import { ConnectionsPage } from "./pages/connections/ConnectionsPage.tsx";
import { ChangeRequestsPage } from "./pages/change-requests/ChangeRequestsPage.tsx";
import { WorkflowsPage } from "./pages/workflows/WorkflowsPage.tsx";
import { IssuesPage } from "./pages/issues/IssuesPage.tsx";
import { RepositoriesPage } from "./pages/repositories/RepositoriesPage.tsx";
import { SettingsPage } from "./pages/settings/SettingsPage.tsx";
import { MonitoringSettingsProvider } from "./api/MonitoringSettingsProvider.tsx";
import { FeatureGate } from "./components/FeatureGate.tsx";

export function App() {
  return (
    <MonitoringSettingsProvider>
      <Routes>
        <Route element={<AppShell />}>
          <Route index element={<WorkflowsPage />} />
          <Route path="activity" element={<ActivityPage />} />
          <Route
            path="pull-requests"
            element={
              <FeatureGate feature="pullRequestsEnabled" label="Pull request">
                <ChangeRequestsPage />
              </FeatureGate>
            }
          />
          <Route
            path="issues"
            element={
              <FeatureGate feature="issuesEnabled" label="Issue">
                <IssuesPage />
              </FeatureGate>
            }
          />
          <Route path="repositories" element={<RepositoriesPage />} />
          <Route path="connections" element={<ConnectionsPage />} />
          <Route path="settings" element={<SettingsPage />} />
          <Route path="*" element={<Navigate to="/" replace />} />
        </Route>
      </Routes>
    </MonitoringSettingsProvider>
  );
}

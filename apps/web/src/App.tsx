import { Navigate, Route, Routes } from "react-router-dom";
import { AppShell } from "./components/AppShell.tsx";
import { ConnectionsPage } from "./pages/connections/ConnectionsPage.tsx";
import { OverviewPage } from "./pages/overview/OverviewPage.tsx";
import { RepositoriesPage } from "./pages/repositories/RepositoriesPage.tsx";

export function App() {
  return (
    <Routes>
      <Route element={<AppShell />}>
        <Route index element={<OverviewPage />} />
        <Route path="repositories" element={<RepositoriesPage />} />
        <Route path="connections" element={<ConnectionsPage />} />
        <Route path="*" element={<Navigate to="/" replace />} />
      </Route>
    </Routes>
  );
}

import { useCallback, useEffect, useState } from "react";
import { useApplicationClient } from "../../api/application-client.tsx";
import { EmptyState } from "../../components/EmptyState.tsx";
import { PageHeader } from "../../components/PageHeader.tsx";
import { PanelHeader } from "../../components/PanelHeader.tsx";
import { groupWorkflows, ProjectGroup } from "../../components/ProjectGroup.tsx";
import type { ListWorkflowsResponse } from "../../generated/contracts.ts";

export function OverviewPage() {
  const client = useApplicationClient();
  const [inventory, setInventory] = useState<ListWorkflowsResponse | null>(null);
  const [error, setError] = useState(false);
  const [loading, setLoading] = useState(true);

  const load = useCallback(async () => {
    setLoading(true);
    setError(false);
    try {
      setInventory(await client.listWorkflows());
    } catch {
      setError(true);
    } finally {
      setLoading(false);
    }
  }, [client]);

  useEffect(() => {
    void load();
  }, [load]);

  const workflows = inventory?.workflows ?? [];
  const projects = groupWorkflows(workflows);
  const active = workflows.filter((workflow) => workflow.state === "active").length;
  const disabled = workflows.length - active;
  const heading = error
    ? "Unable to discover workflows"
    : workflows.length > 0
      ? `${active} active workflow${active === 1 ? "" : "s"} discovered`
      : (inventory?.selectedRepositoryCount ?? 0) > 0
        ? "No workflows discovered"
        : "Select repositories to begin";

  return (
    <section className="page-view" aria-labelledby="overview-title">
      <PageHeader
        eyebrow="Monitored repositories"
        title="Overview"
        description="Workflows discovered across your selected repositories."
        actions={
          <button
            className={`secondary-button${loading ? " button-busy" : ""}`}
            type="button"
            disabled={loading}
            onClick={() => void load()}
          >
            <span aria-hidden="true">↻</span>
            Refresh
          </button>
        }
      />

      <section className="health-summary" aria-labelledby="health-heading" aria-busy={loading}>
        <div className="health-heading">
          <span
            className={`health-icon ${error ? "health-icon-failing" : workflows.length > 0 ? "health-icon-passing" : "health-icon-idle"}`}
            aria-hidden="true"
          >
            {error ? "!" : workflows.length > 0 ? "✓" : "·"}
          </span>
          <div>
            <p className="section-label">Workflow inventory</p>
            <h2 id="health-heading">{loading && !inventory ? "Loading workflows" : heading}</h2>
            <p>
              {error
                ? "Check that the application service is running"
                : inventory
                  ? `Updated just now · ${inventory.selectedRepositoryCount} selected repositor${inventory.selectedRepositoryCount === 1 ? "y" : "ies"}`
                  : "Waiting for the application core"}
            </p>
          </div>
        </div>
        <dl className="health-stats">
          <div><dt>Total</dt><dd>{error ? "—" : workflows.length}</dd></div>
          <div><dt>Active</dt><dd>{error ? "—" : active}</dd></div>
          <div><dt>Disabled</dt><dd>{error ? "—" : disabled}</dd></div>
        </dl>
      </section>

      <div className="overview-content">
        <section className="panel workflows-panel" aria-labelledby="workflows-heading">
          <PanelHeader
            label="Workflows"
            title="Projects and workflows"
            metadata={<span className="panel-badge">{error ? "Unavailable" : `${projects.length} project${projects.length === 1 ? "" : "s"}`}</span>}
          />
          <div className="workflow-list" aria-live="polite">
            {loading && !inventory ? (
              <div className="loading-row">
                <span className="loading-block loading-avatar" />
                <span className="loading-block loading-copy" />
              </div>
            ) : error ? (
              <EmptyState message="The dashboard could not reach the application core." error />
            ) : !inventory || inventory.selectedRepositoryCount === 0 ? (
              <EmptyState message="Select repositories to discover workflows." />
            ) : workflows.length === 0 ? (
              <EmptyState message="No workflows were found in the selected repositories." />
            ) : (
              projects.map((project) => <ProjectGroup key={project.id} project={project} />)
            )}
          </div>
        </section>
      </div>
    </section>
  );
}

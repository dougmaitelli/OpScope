import { useEffect, useMemo, useRef, useState } from "react";
import { useApplicationClient } from "../../api/application-client.tsx";
import { StatusPill } from "../../components/StatusPill.tsx";
import { changeRequestStatusTone } from "../../shared/change-request-status.ts";
import { DialogCloseButton } from "../../components/DialogCloseButton.tsx";
import { MarkdownContent } from "../../components/MarkdownContent.tsx";
import { SourceActions } from "../../components/SourceActions.tsx";
import {
  WorkflowRunLogsDialog,
  type WorkflowRunReference,
} from "../../components/WorkflowRunLogsDialog.tsx";
import type {
  ChangeRequestDetailsResponse,
  ChangeRequestSummary,
  WorkflowRunSummary,
  WorkflowSummary,
} from "../../generated/contracts.ts";
import { requestErrorMessage } from "../../shared/errors.ts";
import { TimeDisplay } from "../../components/TimeDisplay.tsx";
import { CommitLink } from "../../components/CommitLink.tsx";
import { updateWorkflowRun } from "../../shared/action-refresh.ts";
import "./ChangeRequestDetailsDialog.css";

const statusLabel = (value: string) =>
  value.replace(/([A-Z])/g, " $1").replace(/^./, (character) => character.toUpperCase());

export function ChangeRequestDetailsDialog({
  changeRequest: initialChangeRequest,
  onClose,
  onUpdated,
}: {
  changeRequest: ChangeRequestSummary;
  onClose: () => void;
  onUpdated?: (changeRequest: ChangeRequestSummary) => void;
}) {
  const client = useApplicationClient();
  const dialog = useRef<HTMLDialogElement>(null);
  const [changeRequest, setChangeRequest] = useState(initialChangeRequest);
  const [details, setDetails] = useState<ChangeRequestDetailsResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [workflows, setWorkflows] = useState<WorkflowSummary[]>([]);
  const [selectedRun, setSelectedRun] = useState<{
    workflow: WorkflowSummary;
    run: WorkflowRunSummary;
  } | null>(null);
  const [selectedRunReference, setSelectedRunReference] = useState<WorkflowRunReference | null>(
    null,
  );

  useEffect(() => {
    if (dialog.current && !dialog.current.open) dialog.current.showModal();
  }, []);
  useEffect(() => {
    let active = true;
    void client
      .changeRequestDetails({
        sourceId: changeRequest.sourceId,
        repositoryId: changeRequest.repositoryId,
        number: changeRequest.number,
      })
      .then((response) => {
        if (active) setDetails(response);
      })
      .catch((failure: unknown) => {
        if (active) setError(requestErrorMessage(failure));
      })
      .finally(() => {
        if (active) setLoading(false);
      });
    return () => {
      active = false;
    };
  }, [changeRequest.number, changeRequest.repositoryId, changeRequest.sourceId, client]);

  useEffect(() => {
    let active = true;
    void client
      .listWorkflows()
      .then((response) => {
        if (active) setWorkflows(response.workflows);
      })
      .catch(() => {
        // Checks retain their provider links when cached workflow data is unavailable.
      });
    return () => {
      active = false;
    };
  }, [client]);

  const workflowRuns = useMemo(() => {
    const lookup = new Map<string, { workflow: WorkflowSummary; run: WorkflowRunSummary }>();
    for (const workflow of workflows) {
      if (
        workflow.sourceId !== changeRequest.sourceId ||
        workflow.repositoryId !== changeRequest.repositoryId
      )
        continue;
      for (const run of workflow.runs) lookup.set(run.id, { workflow, run });
    }
    return lookup;
  }, [changeRequest.repositoryId, changeRequest.sourceId, workflows]);

  return (
    <>
      <dialog
        className="change-request-dialog"
        ref={dialog}
        aria-labelledby="change-request-dialog-title"
        onCancel={(event) => {
          event.preventDefault();
          dialog.current?.close();
        }}
        onClick={(event) => {
          const bounds = event.currentTarget.getBoundingClientRect();
          if (
            event.clientX < bounds.left ||
            event.clientX > bounds.right ||
            event.clientY < bounds.top ||
            event.clientY > bounds.bottom
          )
            event.currentTarget.close();
        }}
        onClose={onClose}
      >
        <header className="change-request-dialog-header">
          <div>
            <p className="eyebrow">
              {changeRequest.repositoryOwner}/{changeRequest.repositoryName} · #
              {changeRequest.number}
            </p>
            <h2 id="change-request-dialog-title">{changeRequest.title}</h2>
            <p className="change-request-dialog-meta">
              {changeRequest.author ?? "Unknown author"} · {changeRequest.sourceBranch} →{" "}
              {changeRequest.targetBranch}
            </p>
          </div>
          <div className="change-request-dialog-actions">
            <a
              className="secondary-button compact-button"
              href={changeRequest.webUrl}
              target="_blank"
              rel="noreferrer"
            >
              Open in {changeRequest.sourceName}
            </a>
            <DialogCloseButton
              label="Close pull request details"
              onClick={() => dialog.current?.close()}
            />
          </div>
        </header>
        <SourceActions
          sourceId={changeRequest.sourceId}
          repositoryId={changeRequest.repositoryId}
          target={{ type: "changeRequest", number: changeRequest.number }}
          onAccepted={(response) => {
            if (response.details) {
              setDetails(response.details);
              setError(null);
            }
            if (response.changeRequest) {
              setChangeRequest(response.changeRequest);
              onUpdated?.(response.changeRequest);
            }
          }}
        />
        <div className="change-request-dialog-statuses">
          <StatusPill tone={changeRequestStatusTone[changeRequest.reviewStatus]}>
            {changeRequest.reviewStatus === "none" ? (
              "No reviews"
            ) : (
              <>Review · {statusLabel(changeRequest.reviewStatus)}</>
            )}
          </StatusPill>
          <StatusPill tone={changeRequestStatusTone[changeRequest.checkStatus]}>
            {changeRequest.checkStatus === "none" ? (
              "No checks"
            ) : (
              <>Checks · {statusLabel(changeRequest.checkStatus)}</>
            )}
          </StatusPill>
          <StatusPill tone={changeRequestStatusTone[changeRequest.mergeStatus]}>
            Merge · {statusLabel(changeRequest.mergeStatus)}
          </StatusPill>
          <span>
            Updated <TimeDisplay dateTime={changeRequest.updatedAt} />
          </span>
        </div>
        <div className="change-request-dialog-content" aria-busy={loading}>
          {loading ? (
            <p className="change-request-dialog-message">Loading pull request details…</p>
          ) : null}
          {error ? (
            <div className="change-request-dialog-message change-request-dialog-error">
              <strong>Details could not be loaded</strong>
              <span>{error}</span>
            </div>
          ) : null}
          {!loading && !error && details ? (
            <>
              <div className="change-request-dialog-main">
                <section>
                  <h3>Description</h3>
                  {details.body ? (
                    <MarkdownContent content={details.body} baseUrl={changeRequest.webUrl} />
                  ) : (
                    <p className="change-request-empty">No description provided.</p>
                  )}
                </section>
                <section>
                  <h3>
                    Checks <span>{details.checks.length}</span>
                  </h3>
                  {details.checks.length > 0 ? (
                    <div className="change-request-checks">
                      {details.checks.map((check, index) => {
                        const runId = check.workflowRunId;
                        const workflowRun = runId ? workflowRuns.get(runId) : undefined;
                        const content = (
                          <>
                            <span>{check.name}</span>
                            <StatusPill tone={changeRequestStatusTone[check.status]}>
                              {statusLabel(check.status)}
                            </StatusPill>
                          </>
                        );
                        return runId ? (
                          <button
                            type="button"
                            key={`${check.name}:${index}`}
                            onClick={() => {
                              if (workflowRun) {
                                setSelectedRun(workflowRun);
                              } else {
                                setSelectedRunReference({
                                  sourceId: changeRequest.sourceId,
                                  sourceName: changeRequest.sourceName,
                                  repositoryId: changeRequest.repositoryId,
                                  repositoryOwner: changeRequest.repositoryOwner,
                                  repositoryName: changeRequest.repositoryName,
                                  runId,
                                  label: check.name,
                                  webUrl: check.webUrl,
                                });
                              }
                            }}
                            title="Open workflow run details"
                          >
                            {content}
                          </button>
                        ) : check.webUrl ? (
                          <a
                            href={check.webUrl}
                            target="_blank"
                            rel="noreferrer"
                            key={`${check.name}:${index}`}
                          >
                            {content}
                          </a>
                        ) : (
                          <div key={`${check.name}:${index}`}>{content}</div>
                        );
                      })}
                    </div>
                  ) : (
                    <p className="change-request-empty">No individual checks were returned.</p>
                  )}
                </section>
              </div>
              <aside className="change-request-dialog-sidebar">
                <section>
                  <h3>Labels</h3>
                  {details.labels.length > 0 ? (
                    <div className="change-request-labels">
                      {details.labels.map((label) => (
                        <span key={label}>{label}</span>
                      ))}
                    </div>
                  ) : (
                    <p className="change-request-empty">None</p>
                  )}
                </section>
                <section>
                  <h3>Reviews</h3>
                  {details.reviews.length > 0 ? (
                    <div className="change-request-reviews">
                      {details.reviews.map((review, index) => (
                        <div key={`${review.reviewer ?? "unknown"}:${review.submittedAt ?? index}`}>
                          <span>{review.reviewer ?? "Unknown reviewer"}</span>
                          <StatusPill tone={changeRequestStatusTone[review.status]}>
                            {statusLabel(review.status)}
                          </StatusPill>
                        </div>
                      ))}
                    </div>
                  ) : (
                    <p className="change-request-empty">No reviews submitted.</p>
                  )}
                </section>
                {details.latestCommit ? (
                  <section>
                    <h3>Latest commit</h3>
                    <div className="change-request-commit">
                      <code>
                        <CommitLink
                          sha={details.latestCommit.sha}
                          resourceUrl={changeRequest.webUrl}
                        />
                      </code>
                      <strong>{details.latestCommit.title}</strong>
                      <span>
                        {details.latestCommit.author ?? "Unknown author"} ·{" "}
                        <TimeDisplay dateTime={details.latestCommit.committedAt} />
                      </span>
                    </div>
                  </section>
                ) : null}
              </aside>
            </>
          ) : null}
        </div>
      </dialog>
      {selectedRun ? (
        <WorkflowRunLogsDialog
          workflow={selectedRun.workflow}
          run={selectedRun.run}
          onUpdated={(run) =>
            setWorkflows((current) => updateWorkflowRun(current, selectedRun.workflow, run))
          }
          onClose={() => setSelectedRun(null)}
        />
      ) : selectedRunReference ? (
        <WorkflowRunLogsDialog
          reference={selectedRunReference}
          onClose={() => setSelectedRunReference(null)}
        />
      ) : null}
    </>
  );
}

import { useEffect, useRef, useState } from "react";
import { useApplicationClient } from "../api/application-client.tsx";
import type {
  WorkflowRunLogFile,
  WorkflowRunSummary,
  WorkflowSummary,
} from "../generated/contracts.ts";
import { requestErrorMessage } from "../shared/errors.ts";
import { DialogCloseButton } from "./DialogCloseButton.tsx";
import { WorkflowRunState } from "./WorkflowRunState.tsx";
import { SourceActions } from "./SourceActions.tsx";
import "./WorkflowRunLogsDialog.css";

export interface WorkflowRunReference {
  sourceId: string;
  sourceName: string;
  repositoryId: string;
  repositoryOwner: string;
  repositoryName: string;
  runId: string;
  label: string;
  webUrl: string | null;
}

type WorkflowRunLogsDialogProps = (
  | { workflow: WorkflowSummary; run: WorkflowRunSummary; reference?: never }
  | { workflow?: never; run?: never; reference: WorkflowRunReference }
) & { onClose: () => void; onUpdated?: (run: WorkflowRunSummary) => void };

export function WorkflowRunLogsDialog({
  workflow,
  run,
  reference,
  onClose,
  onUpdated,
}: WorkflowRunLogsDialogProps) {
  const client = useApplicationClient();
  const dialog = useRef<HTMLDialogElement>(null);
  const [resolvedRun, setResolvedRun] = useState<WorkflowRunSummary | null>(run ?? null);
  const [files, setFiles] = useState<WorkflowRunLogFile[]>([]);
  const [selectedFile, setSelectedFile] = useState(0);
  const [truncated, setTruncated] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [refreshedRun, setRefreshedRun] = useState<WorkflowRunSummary | null>(null);

  useEffect(() => {
    if (dialog.current && !dialog.current.open) {
      dialog.current.showModal();
    }
  }, []);

  useEffect(() => {
    let active = true;
    const sourceId = workflow?.sourceId ?? reference?.sourceId;
    const repositoryId = workflow?.repositoryId ?? reference?.repositoryId;
    const runId = refreshedRun?.id ?? run?.id ?? reference?.runId;
    if (!sourceId || !repositoryId || !runId) {
      setError("The workflow run reference is incomplete.");
      setLoading(false);
      return;
    }
    setResolvedRun(refreshedRun ?? run ?? null);
    setLoading(true);
    setError(null);
    client
      .workflowRunLogs({
        sourceId,
        repositoryId,
        runId,
        attempt: refreshedRun?.attempt ?? run?.attempt ?? null,
      })
      .then((response) => {
        if (!active) return;
        setResolvedRun(response.run);
        setFiles(response.files);
        setTruncated(response.truncated);
        setSelectedFile(0);
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
  }, [
    client,
    reference?.repositoryId,
    reference?.runId,
    reference?.sourceId,
    run,
    workflow?.sourceId,
    workflow?.repositoryId,
    refreshedRun,
  ]);

  const activeFile = files[selectedFile] ?? null;
  const sourceName = workflow?.sourceName ?? reference?.sourceName ?? "source";
  const repositoryOwner = workflow?.repositoryOwner ?? reference?.repositoryOwner ?? "Unknown";
  const repositoryName = workflow?.repositoryName ?? reference?.repositoryName ?? "repository";
  const workflowName = workflow?.name ?? reference?.label ?? "Workflow run";
  const webUrl = resolvedRun?.webUrl ?? reference?.webUrl;

  return (
    <dialog
      className="run-logs-dialog"
      ref={dialog}
      onCancel={(event) => {
        event.preventDefault();
        dialog.current?.close();
      }}
      onClick={(event) => {
        const bounds = event.currentTarget.getBoundingClientRect();
        const outside =
          event.clientX < bounds.left ||
          event.clientX > bounds.right ||
          event.clientY < bounds.top ||
          event.clientY > bounds.bottom;
        if (outside) {
          event.currentTarget.close();
        }
      }}
      onClose={onClose}
      aria-labelledby="run-logs-title"
    >
      <header className="run-logs-header">
        <div>
          <p className="eyebrow">
            {repositoryOwner}/{repositoryName} · {workflowName}
          </p>
          <h2 id="run-logs-title">
            {resolvedRun ? `#${resolvedRun.runNumber} · ${resolvedRun.title}` : workflowName}
          </h2>
          <p className="run-logs-metadata">
            {resolvedRun
              ? `${resolvedRun.branch ?? "detached"} · ${resolvedRun.commitSha.slice(0, 7)} · attempt ${resolvedRun.attempt}`
              : "Resolving workflow run…"}
          </p>
        </div>
        <div className="run-logs-header-actions">
          {resolvedRun ? <WorkflowRunState run={resolvedRun} /> : null}
          {webUrl ? (
            <a
              className="secondary-button compact-button"
              href={webUrl}
              target="_blank"
              rel="noreferrer"
            >
              Open in {sourceName}
            </a>
          ) : null}
          <DialogCloseButton label="Close logs" onClick={() => dialog.current?.close()} />
        </div>
      </header>

      <div>
        <SourceActions
          sourceId={workflow?.sourceId ?? reference?.sourceId ?? ""}
          repositoryId={workflow?.repositoryId ?? reference?.repositoryId ?? ""}
          target={{
            type: "workflowRun",
            runId: refreshedRun?.id ?? run?.id ?? reference?.runId ?? "",
          }}
          onAccepted={(response) => {
            if (response.run) {
              setRefreshedRun(response.run);
              setResolvedRun(response.run);
              onUpdated?.(response.run);
            }
          }}
        />
        {truncated ? (
          <p className="run-logs-notice">
            This log bundle was shortened to keep the viewer responsive. Open it in {sourceName} for
            the complete output.
          </p>
        ) : null}
      </div>
      <div className="run-logs-content" aria-busy={loading}>
        {loading ? <p className="run-logs-message">Loading run logs…</p> : null}
        {error ? (
          <div className="run-logs-message run-logs-error">
            <strong>Logs could not be loaded</strong>
            <span>{error}</span>
          </div>
        ) : null}
        {!loading && !error && files.length === 0 ? (
          <p className="run-logs-message">This run did not return any log files.</p>
        ) : null}
        {!loading && !error && activeFile ? (
          <>
            <nav className="run-log-files" aria-label="Log files">
              {files.map((file, index) => (
                <button
                  className={index === selectedFile ? "active" : undefined}
                  type="button"
                  key={`${file.name}:${index}`}
                  onClick={() => setSelectedFile(index)}
                  aria-pressed={index === selectedFile}
                  title={file.name}
                >
                  {file.name}
                </button>
              ))}
            </nav>
            <section className="run-log-output" aria-label={activeFile.name}>
              <div className="run-log-output-title">{activeFile.name}</div>
              <pre>{activeFile.content}</pre>
            </section>
          </>
        ) : null}
      </div>
    </dialog>
  );
}

import { useEffect, useRef, useState } from "react";
import { useApplicationClient } from "../api/application-client.tsx";
import type {
  WorkflowRunLogFile,
  WorkflowRunSummary,
  WorkflowSummary,
} from "../generated/contracts.ts";
import { requestErrorMessage } from "../shared/errors.ts";
import { WorkflowRunState } from "./WorkflowRunState.tsx";
import "./WorkflowRunLogsDialog.css";

interface WorkflowRunLogsDialogProps {
  workflow: WorkflowSummary;
  run: WorkflowRunSummary;
  onClose: () => void;
}

export function WorkflowRunLogsDialog({
  workflow,
  run,
  onClose,
}: WorkflowRunLogsDialogProps) {
  const client = useApplicationClient();
  const dialog = useRef<HTMLDialogElement>(null);
  const [files, setFiles] = useState<WorkflowRunLogFile[]>([]);
  const [selectedFile, setSelectedFile] = useState(0);
  const [truncated, setTruncated] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    if (dialog.current && !dialog.current.open) {
      dialog.current.showModal();
    }
  }, []);

  useEffect(() => {
    let active = true;
    setLoading(true);
    setError(null);
    client
      .workflowRunLogs({
        sourceId: workflow.sourceId,
        repositoryId: workflow.repositoryId,
        runId: run.id,
        attempt: run.attempt,
      })
      .then((response) => {
        if (!active) return;
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
  }, [client, run.attempt, run.id, workflow.repositoryId, workflow.sourceId]);

  const activeFile = files[selectedFile] ?? null;

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
            {workflow.repositoryOwner}/{workflow.repositoryName} · {workflow.name}
          </p>
          <h2 id="run-logs-title">
            #{run.runNumber} · {run.title}
          </h2>
          <p className="run-logs-metadata">
            {run.branch ?? "detached"} · {run.commitSha.slice(0, 7)} · attempt {run.attempt}
          </p>
        </div>
        <div className="run-logs-header-actions">
          <WorkflowRunState run={run} />
          <a
            className="secondary-button compact-button"
            href={run.webUrl}
            target="_blank"
            rel="noreferrer"
          >
            Open in {workflow.sourceName}
          </a>
          <button
            className="run-logs-close"
            type="button"
            aria-label="Close logs"
            onClick={() => dialog.current?.close()}
          >
            ×
          </button>
        </div>
      </header>

      {truncated ? (
        <p className="run-logs-notice">
          This log bundle was shortened to keep the viewer responsive. Open it in{" "}
          {workflow.sourceName} for the complete output.
        </p>
      ) : null}

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

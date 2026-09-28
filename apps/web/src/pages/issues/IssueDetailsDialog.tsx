import { useEffect, useRef, useState } from "react";
import { useApplicationClient } from "../../api/application-client.tsx";
import { DialogCloseButton } from "../../components/DialogCloseButton.tsx";
import type { IssueDetailsResponse, IssueSummary } from "../../generated/contracts.ts";
import { requestErrorMessage } from "../../shared/errors.ts";
import { formatRelativeDate } from "../../shared/workflow-runs.ts";
import "./IssueDetailsDialog.css";

export function IssueDetailsDialog({
  issue,
  onClose,
}: {
  issue: IssueSummary;
  onClose: () => void;
}) {
  const client = useApplicationClient();
  const dialog = useRef<HTMLDialogElement>(null);
  const [details, setDetails] = useState<IssueDetailsResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    if (dialog.current && !dialog.current.open) dialog.current.showModal();
  }, []);

  useEffect(() => {
    let active = true;
    void client
      .issueDetails({
        sourceId: issue.sourceId,
        repositoryId: issue.repositoryId,
        number: issue.number,
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
  }, [client, issue.number, issue.repositoryId, issue.sourceId]);

  const current = details ?? issue;

  return (
    <dialog
      className="issue-dialog"
      ref={dialog}
      aria-labelledby="issue-dialog-title"
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
      <header className="issue-dialog-header">
        <div>
          <p className="eyebrow">
            {issue.repositoryOwner}/{issue.repositoryName} · #{issue.number}
          </p>
          <h2 id="issue-dialog-title">{current.title}</h2>
          <p className="issue-dialog-meta">
            {issue.author ?? "Unknown author"} · opened {formatRelativeDate(issue.createdAt)}
          </p>
        </div>
        <div className="issue-dialog-actions">
          <a
            className="secondary-button compact-button"
            href={issue.webUrl}
            target="_blank"
            rel="noreferrer"
          >
            Open in {issue.sourceName}
          </a>
          <DialogCloseButton label="Close issue details" onClick={() => dialog.current?.close()} />
        </div>
      </header>
      <div className="issue-dialog-statuses">
        <span className={`issue-state-${current.state}`}>
          {current.state === "open" ? "Open" : "Closed"}
        </span>
        <span>{current.commentCount} comments</span>
        <span>Updated {formatRelativeDate(current.updatedAt)}</span>
      </div>
      <div className="issue-dialog-content" aria-busy={loading}>
        {loading ? <p className="issue-dialog-message">Loading issue details…</p> : null}
        {error ? (
          <div className="issue-dialog-message issue-dialog-error">
            <strong>Details could not be loaded</strong>
            <span>{error}</span>
          </div>
        ) : null}
        {!loading && !error && details ? (
          <>
            <div className="issue-dialog-main">
              <section>
                <h3>Description</h3>
                {details.body ? (
                  <p className="issue-body">{details.body}</p>
                ) : (
                  <p className="issue-empty">No description provided.</p>
                )}
              </section>
              <section>
                <h3>
                  Recent comments <span>{details.comments.length}</span>
                </h3>
                {details.commentCount > details.comments.length ? (
                  <p className="issue-empty">
                    Showing the latest {details.comments.length} of {details.commentCount} comments.
                  </p>
                ) : null}
                {details.comments.length > 0 ? (
                  <div className="issue-comment-list">
                    {details.comments.map((comment) => (
                      <article key={comment.id}>
                        <header>
                          <strong>{comment.author ?? "Unknown author"}</strong>
                          <span>{formatRelativeDate(comment.createdAt)}</span>
                        </header>
                        <p>{comment.body}</p>
                      </article>
                    ))}
                  </div>
                ) : (
                  <p className="issue-empty">No comments yet.</p>
                )}
              </section>
            </div>
            <aside className="issue-dialog-sidebar">
              <section>
                <h3>Assignees</h3>
                <p className="issue-sidebar-value">
                  {current.assignees.length > 0 ? current.assignees.join(", ") : "Unassigned"}
                </p>
              </section>
              <section>
                <h3>Milestone</h3>
                <p className="issue-sidebar-value">{details.milestone ?? "None"}</p>
              </section>
              <section>
                <h3>Labels</h3>
                {current.labels.length > 0 ? (
                  <div className="issue-dialog-labels">
                    {current.labels.map((label) => (
                      <span key={label}>{label}</span>
                    ))}
                  </div>
                ) : (
                  <p className="issue-empty">None</p>
                )}
              </section>
            </aside>
          </>
        ) : null}
      </div>
    </dialog>
  );
}

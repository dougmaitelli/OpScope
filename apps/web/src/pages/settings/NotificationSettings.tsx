import type { NotificationPreferencesContract } from "../../generated/contracts.ts";
import "./NotificationSettings.css";

const groups: { title: string; events: [keyof NotificationPreferencesContract, string][] }[] = [
  { title: "Workflows", events: [["workflowFailures", "Latest run fails"]] },
  {
    title: "Pull requests",
    events: [
      ["pullRequestOpened", "New pull request"],
      ["pullRequestReviewRequested", "Review requested from you"],
      ["pullRequestChangesRequested", "Changes requested"],
      ["pullRequestMerged", "Pull request merged"],
      ["pullRequestClosed", "Pull request closed"],
    ],
  },
  {
    title: "Issues",
    events: [
      ["issueOpened", "New issue"],
      ["issueAssigned", "Issue assigned to you"],
      ["issueReopened", "Issue reopened"],
      ["issueClosed", "Issue closed"],
    ],
  },
];

export function NotificationSettings({
  value,
  disabled,
  pullRequestsEnabled,
  issuesEnabled,
  onChange,
}: {
  value: NotificationPreferencesContract;
  disabled: boolean;
  pullRequestsEnabled: boolean;
  issuesEnabled: boolean;
  onChange(value: NotificationPreferencesContract): void;
}) {
  return (
    <section className="notification-settings" aria-labelledby="notification-settings-title">
      <h3 id="notification-settings-title">Notifications</h3>
      <p>
        Choose which events trigger notifications. “You” is the connection’s token owner. Only show
        my work also filters these alerts. Existing items establish a baseline; enabling an event
        does not replay changes already observed.
      </p>
      <div className="notification-settings-groups">
        {groups.map((group) => (
          <fieldset
            key={group.title}
            disabled={
              disabled ||
              (group.title === "Pull requests" && !pullRequestsEnabled) ||
              (group.title === "Issues" && !issuesEnabled)
            }
          >
            <legend>{group.title}</legend>
            {group.events.map(([key, label]) => (
              <label key={key}>
                <input
                  type="checkbox"
                  checked={value[key]}
                  onChange={(event) => onChange({ ...value, [key]: event.currentTarget.checked })}
                />
                <span>{label}</span>
              </label>
            ))}
          </fieldset>
        ))}
      </div>
    </section>
  );
}

export function FeatureSettings({
  pullRequestsEnabled,
  issuesEnabled,
  disabled,
  onPullRequestsChanged,
  onIssuesChanged,
}: {
  pullRequestsEnabled: boolean;
  issuesEnabled: boolean;
  disabled: boolean;
  onPullRequestsChanged(enabled: boolean): void;
  onIssuesChanged(enabled: boolean): void;
}) {
  return (
    <section aria-label="Enabled features">
      {[
        {
          title: "Pull request monitoring",
          enabled: pullRequestsEnabled,
          onChange: onPullRequestsChanged,
          description:
            "Show pull requests and PR activity, synchronize updates, and enable PR notifications.",
        },
        {
          title: "Issue monitoring",
          enabled: issuesEnabled,
          onChange: onIssuesChanged,
          description: "Show issues, synchronize updates, and enable issue notifications.",
        },
      ].map((feature) => (
        <label key={feature.title} className="settings-field">
          <span className="settings-field-title">{feature.title}</span>
          <span className="settings-field-description">
            {feature.description} Disabling keeps cached data. Re-enabling starts notifications from
            a fresh baseline.
          </span>
          <input
            className="settings-scope-toggle"
            type="checkbox"
            checked={feature.enabled}
            disabled={disabled}
            onChange={(event) => feature.onChange(event.currentTarget.checked)}
          />
        </label>
      ))}
    </section>
  );
}

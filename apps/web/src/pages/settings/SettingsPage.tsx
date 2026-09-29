import { type FormEvent, useEffect, useState } from "react";
import { useApplicationClient } from "../../api/application-client.tsx";
import { PageHeader } from "../../components/PageHeader.tsx";
import { PanelHeader } from "../../components/PanelHeader.tsx";
import { settingsLimits, type MonitoringSettingsResponse } from "../../generated/contracts.ts";
import { requestErrorMessage } from "../../shared/errors.ts";
import "./SettingsPage.css";
import { NotificationSettings } from "./NotificationSettings.tsx";

export function SettingsPage() {
  const client = useApplicationClient();
  const [saved, setSaved] = useState<MonitoringSettingsResponse | null>(null);
  const [synchronizationInterval, setSynchronizationInterval] = useState("");
  const [recentRuns, setRecentRuns] = useState("");
  const [onlyMyWork, setOnlyMyWork] = useState(false);
  const [notifications, setNotifications] = useState<
    MonitoringSettingsResponse["notifications"] | null
  >(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [note, setNote] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    client
      .getSettings()
      .then((settings) => {
        if (!active) return;
        setSaved(settings);
        setSynchronizationInterval(String(settings.synchronizationIntervalSeconds));
        setRecentRuns(String(settings.recentRunsPerWorkflow));
        setOnlyMyWork(settings.onlyMyWork);
        setNotifications(settings.notifications);
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
  }, [client]);

  const synchronizationIntervalValue = Number(synchronizationInterval);
  const recentRunsValue = Number(recentRuns);
  const valid =
    Number.isInteger(synchronizationIntervalValue) &&
    synchronizationIntervalValue >= settingsLimits.synchronizationIntervalSeconds.min &&
    synchronizationIntervalValue <= settingsLimits.synchronizationIntervalSeconds.max &&
    Number.isInteger(recentRunsValue) &&
    recentRunsValue >= settingsLimits.recentRunsPerWorkflow.min &&
    recentRunsValue <= settingsLimits.recentRunsPerWorkflow.max;
  const changed =
    saved !== null &&
    (synchronizationIntervalValue !== saved.synchronizationIntervalSeconds ||
      recentRunsValue !== saved.recentRunsPerWorkflow ||
      onlyMyWork !== saved.onlyMyWork ||
      JSON.stringify(notifications) !== JSON.stringify(saved.notifications));

  const save = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!valid || !notifications) return;
    setSaving(true);
    setError(null);
    setNote(null);
    try {
      const settings = await client.updateSettings({
        synchronizationIntervalSeconds: synchronizationIntervalValue,
        recentRunsPerWorkflow: recentRunsValue,
        onlyMyWork,
        notifications,
      });
      setSaved(settings);
      setSynchronizationInterval(String(settings.synchronizationIntervalSeconds));
      setRecentRuns(String(settings.recentRunsPerWorkflow));
      setOnlyMyWork(settings.onlyMyWork);
      setNotifications(settings.notifications);
      setNote("Monitoring settings saved.");
    } catch (failure) {
      setError(requestErrorMessage(failure));
    } finally {
      setSaving(false);
    }
  };

  const revert = () => {
    if (!saved) return;
    setSynchronizationInterval(String(saved.synchronizationIntervalSeconds));
    setRecentRuns(String(saved.recentRunsPerWorkflow));
    setOnlyMyWork(saved.onlyMyWork);
    setNotifications(saved.notifications);
    setError(null);
    setNote(null);
  };

  return (
    <section className="page-view" aria-labelledby="settings-title">
      <PageHeader
        eyebrow="Application"
        title="Settings"
        description="Control personal scope, notifications, synchronization, and workflow history."
      />

      <section className="panel settings-panel" aria-labelledby="monitoring-settings-title">
        <PanelHeader label="Monitoring" title="Scope, synchronization and history" />
        <form className="settings-form" onSubmit={(event) => void save(event)}>
          <label className="settings-field">
            <span className="settings-field-title">Synchronization interval</span>
            <span className="settings-field-description">
              How often background synchronization checks connected sources.
            </span>
            <span className="settings-number-control">
              <input
                type="number"
                min={settingsLimits.synchronizationIntervalSeconds.min}
                max={settingsLimits.synchronizationIntervalSeconds.max}
                step="1"
                required
                disabled={loading || saving || saved === null}
                value={synchronizationInterval}
                onChange={(event) => {
                  setSynchronizationInterval(event.target.value);
                  setNote(null);
                }}
              />
              <span>seconds</span>
            </span>
            <small>
              {settingsLimits.synchronizationIntervalSeconds.min}–
              {settingsLimits.synchronizationIntervalSeconds.max} seconds
            </small>
          </label>

          <label className="settings-field">
            <span className="settings-field-title">Recent runs per workflow</span>
            <span className="settings-field-description">
              Number of runs available from each workflow on the Workflows page.
            </span>
            <span className="settings-number-control">
              <input
                type="number"
                min={settingsLimits.recentRunsPerWorkflow.min}
                max={settingsLimits.recentRunsPerWorkflow.max}
                step="1"
                required
                disabled={loading || saving || saved === null}
                value={recentRuns}
                onChange={(event) => {
                  setRecentRuns(event.target.value);
                  setNote(null);
                }}
              />
              <span>runs</span>
            </span>
            <small>
              {settingsLimits.recentRunsPerWorkflow.min}–{settingsLimits.recentRunsPerWorkflow.max}{" "}
              runs
            </small>
          </label>

          <label className="settings-field">
            <span className="settings-field-title">Only show my work</span>
            <span className="settings-field-description">
              Show PRs you opened, reviewed, or were asked to review; issues you opened, follow, or
              discussed; and runs linked to your commits or PRs. “You” means each connection’s token
              owner. Also limits desktop and Apprise notifications. Unverified items stay hidden
              until relevance is available.
            </span>
            <input
              className="settings-scope-toggle"
              type="checkbox"
              checked={onlyMyWork}
              disabled={loading || saving || saved === null}
              onChange={(event) => {
                setOnlyMyWork(event.target.checked);
                setNote(null);
              }}
            />
          </label>

          {notifications ? (
            <NotificationSettings
              value={notifications}
              disabled={loading || saving}
              onChange={(value) => {
                setNotifications(value);
                setNote(null);
              }}
            />
          ) : null}

          {error ? <p className="settings-message settings-error">{error}</p> : null}
          {note ? <p className="settings-message settings-success">{note}</p> : null}

          <div className="settings-actions">
            <button
              className="secondary-button"
              type="button"
              disabled={!changed || saving}
              onClick={revert}
            >
              Revert
            </button>
            <button
              className={`primary-button${saving ? " button-busy" : ""}`}
              type="submit"
              disabled={loading || saving || !valid || !changed}
            >
              {saving ? "Saving…" : "Save settings"}
            </button>
          </div>
        </form>
      </section>
    </section>
  );
}

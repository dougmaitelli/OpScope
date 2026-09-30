import { useRef, useState } from "react";
import { useApplicationClient } from "./application-client.tsx";
import type { SynchronizationScope } from "../generated/contracts.ts";
import { requestErrorMessage } from "../shared/errors.ts";

export function useFeatureRefresh(
  scope: Exclude<SynchronizationScope, "all">,
  reload: () => Promise<void>,
) {
  const client = useApplicationClient();
  const submitting = useRef(false);
  const [refreshing, setRefreshing] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);

  async function refresh() {
    if (submitting.current) return;
    submitting.current = true;
    setRefreshing(true);
    setNotice(null);
    try {
      const result = await client.synchronizeSources({ scope });
      await reload();
      if (result.failedRepositoryCount > 0) {
        setNotice("Some repositories could not be refreshed. Cached data remains available.");
      } else if (result.alreadyRunning) {
        setNotice("Synchronization is already running. This page will update when it finishes.");
      }
    } catch (failure) {
      setNotice(requestErrorMessage(failure));
    } finally {
      submitting.current = false;
      setRefreshing(false);
    }
  }

  return { refresh, refreshing, notice };
}

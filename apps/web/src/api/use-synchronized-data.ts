import {
  useCallback,
  useEffect,
  useEffectEvent,
  useRef,
  useState,
  type SetStateAction,
} from "react";
import { useApplicationClient } from "./application-client.tsx";
import type { SynchronizationStatusResponse } from "../generated/contracts.ts";
import { requestErrorMessage } from "../shared/errors.ts";

export const SYNCHRONIZATION_STATUS_POLL_INTERVAL_MS = 10_000;

/** Read cached page data initially and after the coordinator publishes a new snapshot. */
export function useSynchronizedData<T>(read: () => Promise<T>) {
  const client = useApplicationClient();
  const readLatest = useEffectEvent(read);
  const [data, publishData] = useState<T | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [status, setStatus] = useState<SynchronizationStatusResponse | null>(null);
  const revision = useRef(0);
  const setData = useCallback((update: SetStateAction<T | null>) => {
    // A targeted action/manual refresh must not be overwritten by an older read.
    revision.current += 1;
    publishData(update);
  }, []);

  useEffect(() => {
    let stopped = false;
    let polling = false;
    let initial = true;
    let loaded = false;
    let appliedCompletion: number | null | undefined;
    let wasRunning = false;
    const poll = async () => {
      if (polling || stopped) return;
      polling = true;
      try {
        let current: SynchronizationStatusResponse | null = null;
        try {
          current = await client.synchronizationStatus();
        } catch {
          // Status outages must not prevent the initial cached page load.
        }
        if (stopped) return;
        if (current) setStatus(current);
        const changed =
          current &&
          (appliedCompletion === undefined ||
            current.lastCompletedAt !== appliedCompletion ||
            (wasRunning && !current.running));
        if (!loaded || changed) {
          const startedAtRevision = revision.current;
          try {
            const next = await readLatest();
            if (stopped || startedAtRevision !== revision.current) return;
            publishData(next);
            setError(null);
            loaded = true;
            // Advance only after a successful read, so failures retry next poll.
            appliedCompletion = current?.lastCompletedAt;
            wasRunning = current?.running ?? false;
          } catch (failure) {
            if (!stopped && !loaded) setError(requestErrorMessage(failure));
          }
        } else if (current) {
          wasRunning = current.running;
        }
      } finally {
        if (!stopped && initial) setLoading(false);
        initial = false;
        polling = false;
      }
    };
    void poll();
    const interval = window.setInterval(() => void poll(), SYNCHRONIZATION_STATUS_POLL_INTERVAL_MS);
    const onVisible = () => {
      if (document.visibilityState === "visible") void poll();
    };
    window.addEventListener("focus", onVisible);
    document.addEventListener("visibilitychange", onVisible);
    return () => {
      stopped = true;
      window.clearInterval(interval);
      window.removeEventListener("focus", onVisible);
      document.removeEventListener("visibilitychange", onVisible);
    };
  }, [client]);

  return { data, setData, loading, setLoading, error, setError, status };
}

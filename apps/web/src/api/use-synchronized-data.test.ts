import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  SYNCHRONIZATION_STATUS_POLL_INTERVAL_MS,
  useSynchronizedData,
} from "./use-synchronized-data.ts";

const client = vi.hoisted(() => ({ synchronizationStatus: vi.fn(), synchronizeSources: vi.fn() }));
vi.mock("./application-client.tsx", () => ({ useApplicationClient: () => client }));
const status = (lastCompletedAt: number | null, running = false) => ({
  running,
  lastCompletedAt,
  activeSourceCount: running ? 1 : 0,
  lastFailedRepositoryCount: 0,
});

async function tick() {
  await act(() => vi.advanceTimersByTimeAsync(SYNCHRONIZATION_STATUS_POLL_INTERVAL_MS));
}

beforeEach(() => {
  vi.useFakeTimers();
  vi.resetAllMocks();
  client.synchronizationStatus.mockResolvedValue(status(1));
});
afterEach(() => vi.useRealTimers());

describe("synchronized cached page data", () => {
  it("reloads after background completion, not on unchanged polls or by triggering synchronization", async () => {
    const read = vi.fn().mockResolvedValue(["cached"]);
    const { result, unmount } = renderHook(() => useSynchronizedData(read));
    await act(async () => {});
    expect(result.current.data).toEqual(["cached"]);
    await tick();
    expect(read).toHaveBeenCalledTimes(1);
    client.synchronizationStatus.mockResolvedValue(status(2));
    read.mockResolvedValue(["new"]);
    await tick();
    expect(result.current.data).toEqual(["new"]);
    expect(read).toHaveBeenCalledTimes(2);
    expect(client.synchronizeSources).not.toHaveBeenCalled();
    unmount();
    await tick();
    expect(read).toHaveBeenCalledTimes(2);
  });

  it("retains useful data and retries a failed read of a new snapshot", async () => {
    const read = vi.fn().mockResolvedValue(["cached"]);
    const { result } = renderHook(() => useSynchronizedData(read));
    await act(async () => {});
    client.synchronizationStatus.mockResolvedValue(status(2));
    read.mockRejectedValueOnce(new Error("temporary failure"));
    await tick();
    expect(result.current.data).toEqual(["cached"]);
    expect(result.current.error).toBeNull();
    read.mockResolvedValue(["new"]);
    await tick();
    expect(result.current.data).toEqual(["new"]);
    expect(read).toHaveBeenCalledTimes(3);
  });

  it("checks again on window focus and notices a completed initial synchronization", async () => {
    client.synchronizationStatus.mockResolvedValue(status(null, true));
    const read = vi.fn().mockResolvedValue(["initial"]);
    const { result } = renderHook(() => useSynchronizedData(read));
    await act(async () => {});
    client.synchronizationStatus.mockResolvedValue(status(1));
    read.mockResolvedValue(["ready"]);
    await act(async () => window.dispatchEvent(new Event("focus")));
    expect(result.current.data).toEqual(["ready"]);
    expect(read).toHaveBeenCalledTimes(2);
  });

  it("does not overlap reads or overwrite a targeted action with an older snapshot", async () => {
    let resolve!: (value: string[]) => void;
    const read = vi
      .fn()
      .mockResolvedValueOnce(["initial"])
      .mockImplementationOnce(
        () =>
          new Promise<string[]>((done) => {
            resolve = done;
          }),
      );
    const { result } = renderHook(() => useSynchronizedData(read));
    await act(async () => {});
    client.synchronizationStatus.mockResolvedValue(status(2));
    await tick();
    await tick();
    expect(read).toHaveBeenCalledTimes(2);
    act(() => result.current.setData(["targeted"]));
    await act(async () => resolve(["old"]));
    expect(result.current.data).toEqual(["targeted"]);
    read.mockResolvedValue(["latest"]);
    await tick();
    expect(result.current.data).toEqual(["latest"]);
  });
});

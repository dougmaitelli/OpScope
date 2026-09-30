import { act, renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { useFeatureRefresh } from "./use-feature-refresh.ts";

const client = vi.hoisted(() => ({ synchronizeSources: vi.fn() }));
vi.mock("./application-client.tsx", () => ({ useApplicationClient: () => client }));

describe("feature refresh", () => {
  it("prevents duplicate submissions while refreshing and reports partial failures", async () => {
    let complete!: (value: unknown) => void;
    client.synchronizeSources.mockReset().mockReturnValue(
      new Promise((resolve) => {
        complete = resolve;
      }),
    );
    const reload = vi.fn().mockResolvedValue(undefined);
    const { result } = renderHook(() => useFeatureRefresh("pullRequests", reload));
    let pending!: Promise<void>;
    act(() => {
      pending = result.current.refresh();
    });
    expect(result.current.refreshing).toBe(true);
    await act(() => result.current.refresh());
    expect(client.synchronizeSources).toHaveBeenCalledExactlyOnceWith({ scope: "pullRequests" });
    await act(async () => {
      complete({ failedRepositoryCount: 1, alreadyRunning: false });
      await pending;
    });
    expect(reload).toHaveBeenCalledTimes(1);
    expect(result.current.notice).toContain("Cached data remains available");
    expect(result.current.refreshing).toBe(false);
  });

  it("reports errors without clearing existing data", async () => {
    client.synchronizeSources.mockReset().mockRejectedValue(new Error("Offline"));
    const reload = vi.fn();
    const { result } = renderHook(() => useFeatureRefresh("issues", reload));
    await act(() => result.current.refresh());
    expect(result.current.notice).toBe("Offline");
    expect(result.current.refreshing).toBe(false);
    expect(reload).not.toHaveBeenCalled();
  });
});

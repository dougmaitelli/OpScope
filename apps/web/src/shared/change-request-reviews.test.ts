import { describe, expect, it } from "vitest";
import type { ChangeRequestReviewSummary } from "../generated/contracts.ts";
import { latestReviewerReviews } from "./change-request-reviews.ts";

describe("latestReviewerReviews", () => {
  it("keeps the latest status regardless of input order and login casing", () => {
    const old: ChangeRequestReviewSummary = {
      reviewer: "Alice",
      status: "changesRequested",
      submittedAt: "2026-09-01T00:00:00Z",
    };
    const latest: ChangeRequestReviewSummary = {
      reviewer: "alice",
      status: "approved",
      submittedAt: "2026-09-02T00:00:00Z",
    };
    expect(latestReviewerReviews([old, latest])).toEqual([latest]);
    expect(latestReviewerReviews([latest, old])).toEqual([latest]);
  });

  it("preserves different and unidentified reviewers", () => {
    const reviews: ChangeRequestReviewSummary[] = ["alice", "bob", null, null].map((reviewer) => ({
      reviewer,
      status: "approved",
      submittedAt: null,
    }));
    expect(latestReviewerReviews(reviews)).toEqual(reviews);
  });

  it("prefers submitted reviews over undated entries", () => {
    const submitted: ChangeRequestReviewSummary = {
      reviewer: "alice",
      status: "approved",
      submittedAt: "2026-09-02T00:00:00Z",
    };
    const undated: ChangeRequestReviewSummary = { ...submitted, submittedAt: null };
    expect(latestReviewerReviews([submitted, undated])).toEqual([submitted]);
    expect(latestReviewerReviews([undated, submitted])).toEqual([submitted]);
  });
});

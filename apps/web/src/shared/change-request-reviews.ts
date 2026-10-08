import type { ChangeRequestReviewSummary } from "../generated/contracts.ts";

export function latestReviewerReviews(
  reviews: ChangeRequestReviewSummary[],
): ChangeRequestReviewSummary[] {
  const latest = new Map<string, ChangeRequestReviewSummary>();
  for (const review of reviews) {
    if (!review.reviewer) continue;
    const key = review.reviewer.toLowerCase();
    const previous = latest.get(key);
    const submittedAt = review.submittedAt ? Date.parse(review.submittedAt) : -Infinity;
    const previousSubmittedAt = previous?.submittedAt
      ? Date.parse(previous.submittedAt)
      : -Infinity;
    if (!previous || submittedAt >= previousSubmittedAt) latest.set(key, review);
  }

  // Missing authors cannot safely be treated as the same person.
  const shown = new Set<string>();
  return reviews.flatMap((review) => {
    if (!review.reviewer) return [review];
    const key = review.reviewer.toLowerCase();
    if (shown.has(key)) return [];
    shown.add(key);
    return [latest.get(key)!];
  });
}

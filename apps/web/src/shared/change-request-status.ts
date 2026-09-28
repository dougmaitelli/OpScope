import type {
  ChangeRequestCheckStatus,
  ChangeRequestMergeStatus,
  ChangeRequestReviewStatus,
} from "../generated/contracts.ts";

export const changeRequestStatusTone: Record<
  ChangeRequestCheckStatus | ChangeRequestMergeStatus | ChangeRequestReviewStatus,
  "success" | "failure" | "warning" | "neutral"
> = {
  approved: "success",
  passed: "success",
  ready: "success",
  changesRequested: "failure",
  failing: "failure",
  conflicting: "failure",
  reviewRequired: "warning",
  running: "warning",
  blocked: "warning",
  unknown: "neutral",
};

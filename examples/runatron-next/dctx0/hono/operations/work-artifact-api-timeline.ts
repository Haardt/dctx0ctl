import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.WorkEvidence.ArtifactLineage.WorkArtifactApi.timeline",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/artifactlineage/http/work-artifact-api-timeline.ts", export: "registerWorkArtifactApiTimeline" },
  method: "post",
  path: "/api/work-artifact-api/timeline",
  request: { parameters: { workstreamId: { in: "query" }, actorId: { in: "query" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { ArtifactAccessDenied: { status: 403, body: { from: "failure", optionEncoding: "tagged" } }, ArtifactLedgerUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})


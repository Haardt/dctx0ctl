import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.WorkEvidence.ArtifactLineage.WorkArtifactApi.invalidate",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/artifactlineage/http/work-artifact-api-invalidate.ts", export: "registerWorkArtifactApiInvalidate" },
  method: "post",
  path: "/api/work-artifact-api/invalidate",
  request: { parameters: { request: { in: "body" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { WorkArtifactNotFound: { status: 404, body: { from: "failure", optionEncoding: "tagged" } }, ArtifactAccessDenied: { status: 403, body: { from: "failure", optionEncoding: "tagged" } }, ArtifactStateRejected: { status: 409, body: { from: "failure", optionEncoding: "tagged" } }, ArtifactLedgerUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})


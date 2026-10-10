import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.WorkEvidence.ArtifactLineage.WorkArtifactApi.inspect",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/artifactlineage/http/work-artifact-api-inspect.ts", export: "registerWorkArtifactApiInspect" },
  method: "get",
  path: "/api/work-artifact-api/inspect",
  request: { parameters: { artifactId: { in: "query" }, actorId: { in: "query" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { WorkArtifactNotFound: { status: 404, body: { from: "failure", optionEncoding: "tagged" } }, ArtifactAccessDenied: { status: 403, body: { from: "failure", optionEncoding: "tagged" } }, ArtifactLedgerUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})


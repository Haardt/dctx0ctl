import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.ProjectWork.Workstreams.ProjectWorkApi.workstream",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/workstreams/http/project-work-api-workstream.ts", export: "registerProjectWorkApiWorkstream" },
  method: "get",
  path: "/api/project-work-api/workstream",
  request: { parameters: { workstreamId: { in: "query" }, actorId: { in: "query" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { WorkstreamNotFound: { status: 404, body: { from: "failure", optionEncoding: "tagged" } }, ProjectWorkAccessDenied: { status: 403, body: { from: "failure", optionEncoding: "tagged" } }, ProjectWorkPersistenceUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})


import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.ProjectWork.Workstreams.ProjectWorkApi.project",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/workstreams/http/project-work-api-project.ts", export: "registerProjectWorkApiProject" },
  method: "get",
  path: "/api/project-work-api/project",
  request: { parameters: { projectId: { in: "query" }, actorId: { in: "query" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { ProjectNotFound: { status: 404, body: { from: "failure", optionEncoding: "tagged" } }, ProjectWorkAccessDenied: { status: 403, body: { from: "failure", optionEncoding: "tagged" } }, ProjectWorkPersistenceUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})


import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.ProjectWork.Workstreams.ProjectWorkApi.openWorkstream",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/workstreams/http/project-work-api-open-workstream.ts", export: "registerProjectWorkApiOpenWorkstream" },
  method: "post",
  path: "/api/project-work-api/open-workstream",
  request: { parameters: { request: { in: "body" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { ProjectNotFound: { status: 404, body: { from: "failure", optionEncoding: "tagged" } }, TeamNotFound: { status: 404, body: { from: "failure", optionEncoding: "tagged" } }, ProjectWorkAccessDenied: { status: 403, body: { from: "failure", optionEncoding: "tagged" } }, InvalidProjectPlacement: { status: 409, body: { from: "failure", optionEncoding: "tagged" } }, ProjectWorkPersistenceUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } }, ScopeConversationUnavailable: { status: 404, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})


import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.ProjectWork.Workstreams.ProjectWorkApi.addTeamMember",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/workstreams/http/project-work-api-add-team-member.ts", export: "registerProjectWorkApiAddTeamMember" },
  method: "post",
  path: "/api/project-work-api/add-team-member",
  request: { parameters: { request: { in: "body" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { TeamNotFound: { status: 404, body: { from: "failure", optionEncoding: "tagged" } }, ProjectWorkAccessDenied: { status: 403, body: { from: "failure", optionEncoding: "tagged" } }, InvalidProjectPlacement: { status: 409, body: { from: "failure", optionEncoding: "tagged" } }, ProjectWorkPersistenceUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } }, ScopeConversationUnavailable: { status: 404, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})


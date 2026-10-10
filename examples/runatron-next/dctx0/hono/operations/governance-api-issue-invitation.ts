import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.Administration.WorkspaceGovernance.GovernanceApi.issueInvitation",
  family: "chatApi",
  mode: "custom",
  handler: { root: "src/http/operations/governance-api-issue-invitation.ts", export: "registerGovernanceApiIssueInvitation" },
  method: "post",
  path: "/api/governance-api/issue-invitation",
  request: { parameters: { request: { in: "body" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { AdministrationDenied: { status: 403, body: { from: "failure", optionEncoding: "tagged" } }, WorkspaceNotFound: { status: 404, body: { from: "failure", optionEncoding: "tagged" } }, AdministrationPersistenceUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})

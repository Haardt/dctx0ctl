import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.Administration.WorkspaceGovernance.GovernanceApi.removeMember",
  family: "chatApi",
  mode: "custom",
  handler: { root: "src/http/operations/governance-api-remove-member.ts", export: "registerGovernanceApiRemoveMember" },
  method: "post",
  path: "/api/governance-api/remove-member",
  request: { parameters: { request: { in: "body" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { AdministrationDenied: { status: 403, body: { from: "failure", optionEncoding: "tagged" } }, MemberNotFound: { status: 404, body: { from: "failure", optionEncoding: "tagged" } }, LastAdministratorProtected: { status: 409, body: { from: "failure", optionEncoding: "tagged" } }, AdministrationPersistenceUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})

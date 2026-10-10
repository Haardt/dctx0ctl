import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.Administration.WorkspaceGovernance.GovernanceApi.listInvitations",
  family: "chatApi",
  mode: "custom",
  handler: { root: "src/http/operations/governance-api-list-invitations.ts", export: "registerGovernanceApiListInvitations" },
  method: "get",
  path: "/api/governance-api/list-invitations",
  request: { parameters: { actorId: { in: "query" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { AdministrationDenied: { status: 403, body: { from: "failure", optionEncoding: "tagged" } }, AdministrationPersistenceUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})

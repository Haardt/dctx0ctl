import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.Administration.WorkspaceGovernance.GovernanceApi.listMembers",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/workspacegovernance/http/governance-api-list-members.ts", export: "registerGovernanceApiListMembers" },
  method: "get",
  path: "/api/governance-api/list-members",
  request: { parameters: { actorId: { in: "query" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { AdministrationDenied: { status: 403, body: { from: "failure", optionEncoding: "tagged" } }, AdministrationPersistenceUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})


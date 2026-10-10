import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.Administration.WorkspaceGovernance.GovernanceApi.view",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/workspacegovernance/http/governance-api-view.ts", export: "registerGovernanceApiView" },
  method: "get",
  path: "/api/governance-api/view",
  request: { parameters: { actorId: { in: "query" }, workspaceId: { in: "query" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { WorkspaceNotFound: { status: 404, body: { from: "failure", optionEncoding: "tagged" } }, AdministrationDenied: { status: 403, body: { from: "failure", optionEncoding: "tagged" } }, AdministrationPersistenceUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})


import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.Administration.WorkspaceGovernance.GovernanceApi.currentWorkspace",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/workspacegovernance/http/governance-api-current-workspace.ts", export: "registerGovernanceApiCurrentWorkspace" },
  method: "get",
  path: "/api/governance-api/current-workspace",
  request: { parameters: { actorId: { in: "query" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { WorkspaceInitializationUnavailable: { status: 422, body: { from: "failure", optionEncoding: "tagged" } }, AdministrationDenied: { status: 403, body: { from: "failure", optionEncoding: "tagged" } }, AdministrationPersistenceUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})


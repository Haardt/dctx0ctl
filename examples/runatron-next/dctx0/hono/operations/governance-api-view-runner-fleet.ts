import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.Administration.WorkspaceGovernance.GovernanceApi.viewRunnerFleet",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/workspacegovernance/http/governance-api-view-runner-fleet.ts", export: "registerGovernanceApiViewRunnerFleet" },
  method: "get",
  path: "/api/governance-api/view-runner-fleet",
  request: { parameters: { actorId: { in: "query" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { AdministrationDenied: { status: 403, body: { from: "failure", optionEncoding: "tagged" } }, RunnerFleetUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})

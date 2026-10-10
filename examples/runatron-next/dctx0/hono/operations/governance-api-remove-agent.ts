import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.Administration.WorkspaceGovernance.GovernanceApi.removeAgent",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/workspacegovernance/http/governance-api-remove-agent.ts", export: "registerGovernanceApiRemoveAgent" },
  method: "post",
  path: "/api/governance-api/remove-agent",
  request: { parameters: { request: { in: "body" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { AdministrationDenied: { status: 403, body: { from: "failure", optionEncoding: "tagged" } }, AgentMembershipUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})

import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.Administration.WorkspaceGovernance.GovernanceApi.viewAgentParticipations",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/workspacegovernance/http/governance-api-view-agent-participations.ts", export: "registerGovernanceApiViewAgentParticipations" },
  method: "get",
  path: "/api/governance-api/view-agent-participations",
  request: { parameters: { actorId: { in: "query" }, agentId: { in: "query" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { AdministrationDenied: { status: 403, body: { from: "failure", optionEncoding: "tagged" } }, AgentMembershipUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})

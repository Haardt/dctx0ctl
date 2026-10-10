import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.AgentParticipation.CollaborativeAgents.AgentApi.register",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/collaborativeagents/http/agent-api-register.ts", export: "registerAgentApiRegister" },
  method: "post",
  path: "/api/agent-api/register",
  request: { parameters: { request: { in: "body" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { AgentAccessDenied: { status: 403, body: { from: "failure", optionEncoding: "tagged" } }, AgentConfigurationRejected: { status: 409, body: { from: "failure", optionEncoding: "tagged" } }, RunnerAgentUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } }, AgentParticipationPersistenceUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})

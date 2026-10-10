import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.AgentParticipation.CollaborativeAgents.AgentApi.refreshSnapshot",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/collaborativeagents/http/agent-api-refresh-snapshot.ts", export: "registerAgentApiRefreshSnapshot" },
  method: "post",
  path: "/api/agent-api/refresh-snapshot",
  request: { parameters: { request: { in: "body" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { AgentNotFound: { status: 404, body: { from: "failure", optionEncoding: "tagged" } }, RunnerAgentUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } }, AgentConfigurationRejected: { status: 409, body: { from: "failure", optionEncoding: "tagged" } }, AgentParticipationPersistenceUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } },
  },
})

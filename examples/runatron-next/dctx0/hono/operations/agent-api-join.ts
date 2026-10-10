import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.AgentParticipation.CollaborativeAgents.AgentApi.join",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/collaborativeagents/http/agent-api-join.ts", export: "registerAgentApiJoin" },
  method: "post",
  path: "/api/agent-api/join",
  request: { parameters: { request: { in: "body" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { AgentNotFound: { status: 404, body: { from: "failure", optionEncoding: "tagged" } }, AgentAccessDenied: { status: 403, body: { from: "failure", optionEncoding: "tagged" } }, AgentConfigurationRejected: { status: 409, body: { from: "failure", optionEncoding: "tagged" } }, AgentParticipationPersistenceUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})


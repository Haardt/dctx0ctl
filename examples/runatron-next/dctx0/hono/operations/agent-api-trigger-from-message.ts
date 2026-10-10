import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.AgentParticipation.CollaborativeAgents.AgentApi.triggerFromMessage",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/collaborativeagents/http/agent-api-trigger-from-message.ts", export: "registerAgentApiTriggerFromMessage" },
  method: "post",
  path: "/api/agent-api/trigger-from-message",
  request: { parameters: { request: { in: "body" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { AgentParticipationNotFound: { status: 404, body: { from: "failure", optionEncoding: "tagged" } }, AgentTriggerRejected: { status: 409, body: { from: "failure", optionEncoding: "tagged" } }, AgentParticipationPersistenceUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } }, AgentRunQueueUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})

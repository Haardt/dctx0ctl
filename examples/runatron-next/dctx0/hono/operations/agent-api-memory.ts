import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.AgentParticipation.CollaborativeAgents.AgentApi.memory",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/collaborativeagents/http/agent-api-memory.ts", export: "registerAgentApiMemory" },
  method: "get",
  path: "/api/agent-api/memory",
  request: { parameters: { agentId: { in: "query" }, actorId: { in: "query" }, scopeId: { in: "query" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { AgentNotFound: { status: 404, body: { from: "failure", optionEncoding: "tagged" } }, AgentAccessDenied: { status: 403, body: { from: "failure", optionEncoding: "tagged" } }, AgentParticipationPersistenceUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})


import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "RunnerChannelAuthority" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.AgentExecution.InspectableRuns.AgentRunApi.chatContext",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/inspectableruns/http/agent-run-api-chat-context.ts", export: "registerAgentRunApiChatContext" },
  method: "post",
  path: "/api/runner/runs/chat-context",
  request: { parameters: { request: { in: "body" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { RunnerAuthenticationRejected: { status: 401, body: { from: "failure", optionEncoding: "tagged" } }, RunnerLeaseRejected: { status: 409, body: { from: "failure", optionEncoding: "tagged" } }, ChatContextDenied: { status: 403, body: { from: "failure", optionEncoding: "tagged" } }, RunLedgerUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } },
  },
})

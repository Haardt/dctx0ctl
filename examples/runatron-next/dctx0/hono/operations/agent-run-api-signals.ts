import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "RunnerChannelAuthority" as const
export const responseContentTypes = ["text/event-stream"] as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.AgentExecution.InspectableRuns.AgentRunApi.signals",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/inspectableruns/http/agent-run-api-signals.ts", export: "registerAgentRunApiSignals" },
  method: "post",
  path: "/api/runner/signals",
  request: { parameters: { request: { in: "body" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { RunnerAuthenticationRejected: { status: 401, body: { from: "failure", optionEncoding: "tagged" } }, RunLedgerUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } },
  },
})

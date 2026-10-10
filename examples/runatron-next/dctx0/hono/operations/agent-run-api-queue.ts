import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.AgentExecution.InspectableRuns.AgentRunApi.queue",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/inspectableruns/http/agent-run-api-queue.ts", export: "registerAgentRunApiQueue" },
  method: "post",
  path: "/api/agent-runs/queue",
  request: { parameters: { request: { in: "body" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { RunQueueDenied: { status: 403, body: { from: "failure", optionEncoding: "tagged" } }, RunSnapshotUnavailable: { status: 409, body: { from: "failure", optionEncoding: "tagged" } }, InvalidRunTransition: { status: 409, body: { from: "failure", optionEncoding: "tagged" } }, RunLedgerUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } },
  },
})

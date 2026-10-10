import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.AgentExecution.InspectableRuns.AgentRunApi.recover",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/inspectableruns/http/agent-run-api-recover.ts", export: "registerAgentRunApiRecover" },
  method: "post",
  path: "/api/agent-runs/recover",
  request: { parameters: { request: { in: "body" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { AgentRunNotFound: { status: 404, body: { from: "failure", optionEncoding: "tagged" } }, RunnerLeaseRejected: { status: 409, body: { from: "failure", optionEncoding: "tagged" } }, UnconfirmedToolWrite: { status: 409, body: { from: "failure", optionEncoding: "tagged" } }, InvalidRunTransition: { status: 409, body: { from: "failure", optionEncoding: "tagged" } }, RunLedgerUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } },
  },
})

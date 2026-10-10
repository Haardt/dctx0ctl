import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.AgentExecution.InspectableRuns.AgentRunApi.requestControl",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/inspectableruns/http/agent-run-api-request-control.ts", export: "registerAgentRunApiRequestControl" },
  method: "post",
  path: "/api/agent-runs/request-control",
  request: { parameters: { request: { in: "body" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { AgentRunNotFound: { status: 404, body: { from: "failure", optionEncoding: "tagged" } }, RunAttemptNotFound: { status: 404, body: { from: "failure", optionEncoding: "tagged" } }, RunCheckpointNotFound: { status: 404, body: { from: "failure", optionEncoding: "tagged" } }, RunInspectionDenied: { status: 403, body: { from: "failure", optionEncoding: "tagged" } }, RunControlRejected: { status: 409, body: { from: "failure", optionEncoding: "tagged" } }, RunLedgerUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } },
  },
})

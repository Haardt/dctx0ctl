import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.AgentExecution.InspectableRuns.AgentRunApi.inspect",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/inspectableruns/http/agent-run-api-inspect.ts", export: "registerAgentRunApiInspect" },
  method: "get",
  path: "/api/agent-runs/:runId",
  request: { parameters: { runId: { in: "path" }, actorId: { in: "query" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { AgentRunNotFound: { status: 404, body: { from: "failure", optionEncoding: "tagged" } }, RunInspectionDenied: { status: 403, body: { from: "failure", optionEncoding: "tagged" } }, RunLedgerUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } },
  },
})

import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.AgentExecution.InspectableRuns.AgentRunApi.reconcileWritingTool",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/inspectableruns/http/agent-run-api-reconcile-writing-tool.ts", export: "registerAgentRunApiReconcileWritingTool" },
  method: "post",
  path: "/api/agent-runs/reconcile-writing-tool",
  request: { parameters: { request: { in: "body" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { AgentRunNotFound: { status: 404, body: { from: "failure", optionEncoding: "tagged" } }, RunEventNotFound: { status: 404, body: { from: "failure", optionEncoding: "tagged" } }, RunInspectionDenied: { status: 403, body: { from: "failure", optionEncoding: "tagged" } }, InvalidRunTransition: { status: 409, body: { from: "failure", optionEncoding: "tagged" } }, RunLedgerUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } },
  },
})

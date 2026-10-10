import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.AgentExecution.InspectableRuns.AgentRunApi.activities",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/inspectableruns/http/agent-run-api-activities.ts", export: "registerAgentRunApiActivities" },
  method: "get",
  path: "/api/agent-runs/:runId/activities",
  request: {
    parameters: {
      sessionCredential: { in: "header", name: "authorization" },
      runId: { in: "path" },
      query: { in: "query" },
    },
  },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: {
      RunActivityAuthenticationRequired: { status: 401, body: { from: "failure", optionEncoding: "tagged" } },
      AgentRunNotFound: { status: 404, body: { from: "failure", optionEncoding: "tagged" } },
      RunAttemptNotFound: { status: 404, body: { from: "failure", optionEncoding: "tagged" } },
      RunInspectionDenied: { status: 403, body: { from: "failure", optionEncoding: "tagged" } },
      InvalidRunTransition: { status: 422, body: { from: "failure", optionEncoding: "tagged" } },
      RunLedgerUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } },
    },
  },
})

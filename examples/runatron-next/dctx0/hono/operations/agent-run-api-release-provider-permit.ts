import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "RunnerChannelAuthority" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.AgentExecution.InspectableRuns.AgentRunApi.releaseProviderPermit",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/inspectableruns/http/agent-run-api-release-provider-permit.ts", export: "registerAgentRunApiReleaseProviderPermit" },
  method: "post",
  path: "/api/runner/provider-permits/release",
  request: { parameters: { request: { in: "body" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { RunnerAuthenticationRejected: { status: 401, body: { from: "failure", optionEncoding: "tagged" } }, RunnerLeaseRejected: { status: 409, body: { from: "failure", optionEncoding: "tagged" } }, ProviderPermitRejected: { status: 429, body: { from: "failure", optionEncoding: "tagged" } }, RunLedgerUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } },
  },
})

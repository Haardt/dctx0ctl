import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.RunnerInventory.RunnerRegistry.RunnerRegistryApi.revoke",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/runnerregistry/http/runner-registry-api-revoke.ts", export: "registerRunnerRegistryApiRevoke" },
  method: "post",
  path: "/api/runner-registry-api/revoke",
  request: { parameters: { request: { in: "body" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { RunnerNotFound: { status: 404, body: { from: "failure", optionEncoding: "tagged" } }, RunnerAccessDenied: { status: 403, body: { from: "failure", optionEncoding: "tagged" } }, RunnerInventoryUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } },
  },
})

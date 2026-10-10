import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "RunnerChannelAuthority" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.RunnerInventory.RunnerRegistry.RunnerRegistryApi.report",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/runnerregistry/http/runner-registry-api-report.ts", export: "registerRunnerRegistryApiReport" },
  method: "post",
  path: "/api/runner/inventory/report",
  request: { parameters: { request: { in: "body" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { RunnerKeyRejected: { status: 401, body: { from: "failure", optionEncoding: "tagged" } }, InventoryConflict: { status: 409, body: { from: "failure", optionEncoding: "tagged" } }, InventoryRejected: { status: 409, body: { from: "failure", optionEncoding: "tagged" } }, RunnerInventoryUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } },
  },
})

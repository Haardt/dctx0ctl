import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.RunnerInventory.RunnerRegistry.RunnerRegistryApi.provision",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/runnerregistry/http/runner-registry-api-provision.ts", export: "registerRunnerRegistryApiProvision" },
  method: "post",
  path: "/api/runner-registry-api/provision",
  request: { parameters: { request: { in: "body" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { RunnerAccessDenied: { status: 403, body: { from: "failure", optionEncoding: "tagged" } }, InventoryConflict: { status: 409, body: { from: "failure", optionEncoding: "tagged" } }, RunnerInventoryUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } },
  },
})

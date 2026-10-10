import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.RunnerInventory.RunnerRegistry.RunnerRegistryApi.snapshot",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/runnerregistry/http/runner-registry-api-snapshot.ts", export: "registerRunnerRegistryApiSnapshot" },
  method: "get",
  path: "/api/runner-registry-api/snapshot",
  request: { parameters: { snapshotId: { in: "query" }, actorId: { in: "query" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { AgentSnapshotNotFound: { status: 404, body: { from: "failure", optionEncoding: "tagged" } }, RunnerAccessDenied: { status: 403, body: { from: "failure", optionEncoding: "tagged" } }, RunnerInventoryUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } },
  },
})

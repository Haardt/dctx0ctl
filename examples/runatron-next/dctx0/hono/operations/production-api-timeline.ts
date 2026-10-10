import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.ProductionOperations.ProductionFeedback.ProductionApi.timeline",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/productionfeedback/http/production-api-timeline.ts", export: "registerProductionApiTimeline" },
  method: "post",
  path: "/api/production-api/timeline",
  request: { parameters: { workstreamId: { in: "query" }, actorId: { in: "query" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { OperationsAccessDenied: { status: 403, body: { from: "failure", optionEncoding: "tagged" } }, OperationsLedgerUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})


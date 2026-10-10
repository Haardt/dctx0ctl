import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.ProductionOperations.ProductionFeedback.ProductionApi.receiveAlert",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/productionfeedback/http/production-api-receive-alert.ts", export: "registerProductionApiReceiveAlert" },
  method: "post",
  path: "/api/production-api/receive-alert",
  request: { parameters: { request: { in: "body" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { OperationsEvidenceRejected: { status: 409, body: { from: "failure", optionEncoding: "tagged" } }, OperationsLedgerUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})


import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.ProductionOperations.ProductionFeedback.ProductionApi.resolveAlert",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/productionfeedback/http/production-api-resolve-alert.ts", export: "registerProductionApiResolveAlert" },
  method: "post",
  path: "/api/production-api/resolve-alert",
  request: { parameters: { request: { in: "body" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { OperationalAlertNotFound: { status: 404, body: { from: "failure", optionEncoding: "tagged" } }, OperationsAccessDenied: { status: 403, body: { from: "failure", optionEncoding: "tagged" } }, OperationsTransitionRejected: { status: 409, body: { from: "failure", optionEncoding: "tagged" } }, OperationsEvidenceRejected: { status: 409, body: { from: "failure", optionEncoding: "tagged" } }, OperationsLedgerUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})


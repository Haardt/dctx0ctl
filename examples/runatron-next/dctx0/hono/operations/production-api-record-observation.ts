import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.ProductionOperations.ProductionFeedback.ProductionApi.recordObservation",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/productionfeedback/http/production-api-record-observation.ts", export: "registerProductionApiRecordObservation" },
  method: "post",
  path: "/api/production-api/record-observation",
  request: { parameters: { request: { in: "body" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { ObservedDeploymentNotFound: { status: 404, body: { from: "failure", optionEncoding: "tagged" } }, OperationsAccessDenied: { status: 403, body: { from: "failure", optionEncoding: "tagged" } }, OperationsEvidenceRejected: { status: 409, body: { from: "failure", optionEncoding: "tagged" } }, OperationsLedgerUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})


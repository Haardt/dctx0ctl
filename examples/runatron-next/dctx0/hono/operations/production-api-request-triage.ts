import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.ProductionOperations.ProductionFeedback.ProductionApi.requestTriage",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/productionfeedback/http/production-api-request-triage.ts", export: "registerProductionApiRequestTriage" },
  method: "post",
  path: "/api/production-api/request-triage",
  request: { parameters: { request: { in: "body" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { OperationalAlertNotFound: { status: 404, body: { from: "failure", optionEncoding: "tagged" } }, OperationsAccessDenied: { status: 403, body: { from: "failure", optionEncoding: "tagged" } }, OperationalAutomationRejected: { status: 409, body: { from: "failure", optionEncoding: "tagged" } }, OperationsIntegrationUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})


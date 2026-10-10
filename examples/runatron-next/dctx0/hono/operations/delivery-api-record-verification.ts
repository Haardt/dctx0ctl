import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.SoftwareDelivery.DeliveryEvidence.DeliveryApi.recordVerification",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/deliveryevidence/http/delivery-api-record-verification.ts", export: "registerDeliveryApiRecordVerification" },
  method: "post",
  path: "/api/delivery-api/record-verification",
  request: { parameters: { request: { in: "body" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { ChangeSetNotFound: { status: 404, body: { from: "failure", optionEncoding: "tagged" } }, DeliveryAccessDenied: { status: 403, body: { from: "failure", optionEncoding: "tagged" } }, DeliveryEvidenceRejected: { status: 409, body: { from: "failure", optionEncoding: "tagged" } }, DeliveryLedgerUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})


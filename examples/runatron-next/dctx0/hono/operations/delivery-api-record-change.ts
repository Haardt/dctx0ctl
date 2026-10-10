import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.SoftwareDelivery.DeliveryEvidence.DeliveryApi.recordChange",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/deliveryevidence/http/delivery-api-record-change.ts", export: "registerDeliveryApiRecordChange" },
  method: "post",
  path: "/api/delivery-api/record-change",
  request: { parameters: { request: { in: "body" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { DeliveryAccessDenied: { status: 403, body: { from: "failure", optionEncoding: "tagged" } }, DeliveryEvidenceRejected: { status: 409, body: { from: "failure", optionEncoding: "tagged" } }, DeliveryLedgerUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})


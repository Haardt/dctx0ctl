import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.SoftwareDelivery.DeliveryEvidence.DeliveryApi.timeline",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/deliveryevidence/http/delivery-api-timeline.ts", export: "registerDeliveryApiTimeline" },
  method: "post",
  path: "/api/delivery-api/timeline",
  request: { parameters: { workstreamId: { in: "query" }, actorId: { in: "query" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { DeliveryAccessDenied: { status: 403, body: { from: "failure", optionEncoding: "tagged" } }, DeliveryLedgerUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})


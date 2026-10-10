import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.SoftwareDelivery.DeliveryEvidence.DeliveryApi.cancelDeployment",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/deliveryevidence/http/delivery-api-cancel-deployment.ts", export: "registerDeliveryApiCancelDeployment" },
  method: "post",
  path: "/api/delivery-api/cancel-deployment",
  request: { parameters: { request: { in: "body" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { DeploymentNotFound: { status: 404, body: { from: "failure", optionEncoding: "tagged" } }, DeliveryAccessDenied: { status: 403, body: { from: "failure", optionEncoding: "tagged" } }, DeliveryTransitionRejected: { status: 409, body: { from: "failure", optionEncoding: "tagged" } }, DeliveryLedgerUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})


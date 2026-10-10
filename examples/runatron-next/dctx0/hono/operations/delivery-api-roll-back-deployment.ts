import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.SoftwareDelivery.DeliveryEvidence.DeliveryApi.rollBackDeployment",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/deliveryevidence/http/delivery-api-roll-back-deployment.ts", export: "registerDeliveryApiRollBackDeployment" },
  method: "post",
  path: "/api/delivery-api/roll-back-deployment",
  request: { parameters: { request: { in: "body" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { DeploymentNotFound: { status: 404, body: { from: "failure", optionEncoding: "tagged" } }, DeliveryAccessDenied: { status: 403, body: { from: "failure", optionEncoding: "tagged" } }, DeliveryTransitionRejected: { status: 409, body: { from: "failure", optionEncoding: "tagged" } }, DeliveryEvidenceRejected: { status: 409, body: { from: "failure", optionEncoding: "tagged" } }, DeliveryLedgerUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})


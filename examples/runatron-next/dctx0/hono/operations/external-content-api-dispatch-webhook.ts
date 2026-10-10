import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.Automation.BotsAndExternalContent.ExternalContentApi.dispatchWebhook",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/botsandexternalcontent/http/external-content-api-dispatch-webhook.ts", export: "registerExternalContentApiDispatchWebhook" },
  method: "post",
  path: "/api/external-content-api/dispatch-webhook",
  request: { parameters: { automationId: { in: "query" }, message: { in: "body" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { AutomationNotFound: { status: 404, body: { from: "failure", optionEncoding: "tagged" } }, WebhookTargetRejected: { status: 409, body: { from: "failure", optionEncoding: "tagged" } }, WebhookDeliveryFailed: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})


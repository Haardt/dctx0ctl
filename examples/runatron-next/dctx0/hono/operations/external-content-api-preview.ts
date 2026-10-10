import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.Automation.BotsAndExternalContent.ExternalContentApi.preview",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/botsandexternalcontent/http/external-content-api-preview.ts", export: "registerExternalContentApiPreview" },
  method: "post",
  path: "/api/external-content-api/preview",
  request: { parameters: { request: { in: "body" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "nullable" } },
    failures: { AutomationAccessDenied: { status: 403, body: { from: "failure", optionEncoding: "tagged" } }, PreviewTargetRejected: { status: 409, body: { from: "failure", optionEncoding: "tagged" } }, PreviewUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})


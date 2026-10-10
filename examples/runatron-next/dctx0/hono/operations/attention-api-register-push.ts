import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.Attention.PresenceAndNotifications.AttentionApi.registerPush",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/presenceandnotifications/http/attention-api-register-push.ts", export: "registerAttentionApiRegisterPush" },
  method: "post",
  path: "/api/attention-api/register-push",
  request: { parameters: { request: { in: "body" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { PushEndpointRejected: { status: 409, body: { from: "failure", optionEncoding: "tagged" } }, AttentionPersistenceUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})


import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.Attention.PresenceAndNotifications.AttentionApi.removePush",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/presenceandnotifications/http/attention-api-remove-push.ts", export: "registerAttentionApiRemovePush" },
  method: "post",
  path: "/api/attention-api/remove-push",
  request: { parameters: { request: { in: "body" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { PushSubscriptionNotFound: { status: 404, body: { from: "failure", optionEncoding: "tagged" } }, AttentionPersistenceUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})


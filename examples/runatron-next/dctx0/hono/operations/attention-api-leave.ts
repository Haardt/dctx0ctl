import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.Attention.PresenceAndNotifications.AttentionApi.leave",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/presenceandnotifications/http/attention-api-leave.ts", export: "registerAttentionApiLeave" },
  method: "post",
  path: "/api/attention-api/leave",
  request: { parameters: { request: { in: "body" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { ConnectionNotFound: { status: 404, body: { from: "failure", optionEncoding: "tagged" } }, AttentionPersistenceUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})


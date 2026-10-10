import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.Attention.PresenceAndNotifications.AttentionApi.enter",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/presenceandnotifications/http/attention-api-enter.ts", export: "registerAttentionApiEnter" },
  method: "post",
  path: "/api/attention-api/enter",
  request: { parameters: { request: { in: "body" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { AttentionAccessDenied: { status: 403, body: { from: "failure", optionEncoding: "tagged" } }, AttentionPersistenceUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})


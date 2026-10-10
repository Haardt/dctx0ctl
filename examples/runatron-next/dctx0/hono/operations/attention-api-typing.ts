import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.Attention.PresenceAndNotifications.AttentionApi.typing",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/presenceandnotifications/http/attention-api-typing.ts", export: "registerAttentionApiTyping" },
  method: "post",
  path: "/api/attention-api/typing",
  request: { parameters: { request: { in: "body" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { AttentionAccessDenied: { status: 403, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})


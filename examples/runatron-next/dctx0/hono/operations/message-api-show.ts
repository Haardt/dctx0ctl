import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.Messaging.ConversationHistory.MessageApi.show",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/conversationhistory/http/message-api-show.ts", export: "registerMessageApiShow" },
  method: "get",
  path: "/api/message-api/show",
  request: { parameters: { actorId: { in: "query" }, messageId: { in: "query" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { MessageNotFound: { status: 404, body: { from: "failure", optionEncoding: "tagged" } }, MessageAccessDenied: { status: 403, body: { from: "failure", optionEncoding: "tagged" } }, MessagingPersistenceUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})


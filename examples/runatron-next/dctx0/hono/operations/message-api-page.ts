import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.Messaging.ConversationHistory.MessageApi.page",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/conversationhistory/http/message-api-page.ts", export: "registerMessageApiPage" },
  method: "post",
  path: "/api/message-api/page",
  request: { parameters: { request: { in: "body" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { MessageAccessDenied: { status: 403, body: { from: "failure", optionEncoding: "tagged" } }, MessageNotFound: { status: 404, body: { from: "failure", optionEncoding: "tagged" } }, MessagingPersistenceUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})


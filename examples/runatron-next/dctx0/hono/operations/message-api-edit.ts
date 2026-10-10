import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.Messaging.ConversationHistory.MessageApi.edit",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/conversationhistory/http/message-api-edit.ts", export: "registerMessageApiEdit" },
  method: "post",
  path: "/api/message-api/edit",
  request: { parameters: { request: { in: "body" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { MessageNotFound: { status: 404, body: { from: "failure", optionEncoding: "tagged" } }, MessageAccessDenied: { status: 403, body: { from: "failure", optionEncoding: "tagged" } }, MessageMutationDenied: { status: 403, body: { from: "failure", optionEncoding: "tagged" } }, EmptyMessage: { status: 409, body: { from: "failure", optionEncoding: "tagged" } }, MessagingPersistenceUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})


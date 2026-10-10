import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.Messaging.ConversationHistory.MessageApi.publish",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/conversationhistory/http/message-api-publish.ts", export: "registerMessageApiPublish" },
  method: "post",
  path: "/api/message-api/publish",
  request: { parameters: { request: { in: "body" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { MessageAccessDenied: { status: 403, body: { from: "failure", optionEncoding: "tagged" } }, EmptyMessage: { status: 409, body: { from: "failure", optionEncoding: "tagged" } }, InvalidAttachment: { status: 409, body: { from: "failure", optionEncoding: "tagged" } }, DuplicateClientMessageConflict: { status: 409, body: { from: "failure", optionEncoding: "tagged" } }, MessagingPersistenceUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})


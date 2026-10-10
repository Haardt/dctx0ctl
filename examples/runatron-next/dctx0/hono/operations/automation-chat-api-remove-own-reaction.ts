import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "AutomationKey" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.Messaging.ConversationHistory.AutomationChatApi.removeOwnReaction",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/conversationhistory/http/automation-chat-api-remove-own-reaction.ts", export: "registerAutomationChatApiRemoveOwnReaction" },
  method: "post",
  path: "/api/automation-chat-api/remove-own-reaction",
  request: { parameters: { request: { in: "body" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { AutomationKeyRejected: { status: 401, body: { from: "failure", optionEncoding: "tagged" } }, AutomationAccessDenied: { status: 403, body: { from: "failure", optionEncoding: "tagged" } }, AutomationMutationDenied: { status: 403, body: { from: "failure", optionEncoding: "tagged" } }, AutomationPersistenceUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})


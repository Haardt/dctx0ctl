import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "AutomationKey" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.Messaging.ConversationHistory.AutomationChatApi.editOwn",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/conversationhistory/http/automation-chat-api-edit-own.ts", export: "registerAutomationChatApiEditOwn" },
  method: "post",
  path: "/api/automation-chat-api/edit-own",
  request: { parameters: { request: { in: "body" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { AutomationKeyRejected: { status: 401, body: { from: "failure", optionEncoding: "tagged" } }, AutomationAccessDenied: { status: 403, body: { from: "failure", optionEncoding: "tagged" } }, AutomationMutationDenied: { status: 403, body: { from: "failure", optionEncoding: "tagged" } }, AutomationContentRejected: { status: 422, body: { from: "failure", optionEncoding: "tagged" } }, AutomationPersistenceUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})


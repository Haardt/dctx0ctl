import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.Conversations.Spaces.ConversationApi.listVisible",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/spaces/http/conversation-api-list-visible.ts", export: "registerConversationApiListVisible" },
  method: "get",
  path: "/api/conversation-api/list-visible",
  request: { parameters: { personId: { in: "query" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { ConversationPersistenceUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})


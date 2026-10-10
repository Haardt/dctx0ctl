import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.Conversations.Spaces.ConversationApi.authorize",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/spaces/http/conversation-api-authorize.ts", export: "registerConversationApiAuthorize" },
  method: "get",
  path: "/api/conversation-api/authorize",
  request: { parameters: { spaceId: { in: "query" }, personId: { in: "query" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { SpaceNotFound: { status: 404, body: { from: "failure", optionEncoding: "tagged" } }, SpaceAccessDenied: { status: 403, body: { from: "failure", optionEncoding: "tagged" } }, ConversationPersistenceUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})


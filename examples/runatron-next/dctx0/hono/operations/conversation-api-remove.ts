import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.Conversations.Spaces.ConversationApi.remove",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/spaces/http/conversation-api-remove.ts", export: "registerConversationApiRemove" },
  method: "post",
  path: "/api/conversation-api/remove",
  request: { parameters: { request: { in: "body" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { SpaceNotFound: { status: 404, body: { from: "failure", optionEncoding: "tagged" } }, SpaceChangeDenied: { status: 403, body: { from: "failure", optionEncoding: "tagged" } }, ConversationPersistenceUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})


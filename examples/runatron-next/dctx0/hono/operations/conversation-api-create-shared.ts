import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.Conversations.Spaces.ConversationApi.createShared",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/spaces/http/conversation-api-create-shared.ts", export: "registerConversationApiCreateShared" },
  method: "post",
  path: "/api/conversation-api/create-shared",
  request: { parameters: { request: { in: "body" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { SpaceChangeDenied: { status: 403, body: { from: "failure", optionEncoding: "tagged" } }, InvalidParticipantSet: { status: 409, body: { from: "failure", optionEncoding: "tagged" } }, ConversationPersistenceUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})


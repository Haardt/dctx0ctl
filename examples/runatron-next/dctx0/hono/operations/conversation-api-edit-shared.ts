import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.Conversations.Spaces.ConversationApi.editShared",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/spaces/http/conversation-api-edit-shared.ts", export: "registerConversationApiEditShared" },
  method: "post",
  path: "/api/conversation-api/edit-shared",
  request: { parameters: { request: { in: "body" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { SpaceNotFound: { status: 404, body: { from: "failure", optionEncoding: "tagged" } }, SpaceChangeDenied: { status: 403, body: { from: "failure", optionEncoding: "tagged" } }, InvalidParticipantSet: { status: 409, body: { from: "failure", optionEncoding: "tagged" } }, DirectConversationImmutable: { status: 409, body: { from: "failure", optionEncoding: "tagged" } }, ConversationPersistenceUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})


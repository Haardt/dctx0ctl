import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.RealtimeNavigation.LiveConversationDelivery.RealtimeApi.unsubscribe",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/liveconversationdelivery/http/realtime-api-unsubscribe.ts", export: "registerRealtimeApiUnsubscribe" },
  method: "post",
  path: "/api/realtime-api/unsubscribe",
  request: { parameters: { request: { in: "body" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { RealtimeConnectionNotFound: { status: 404, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})


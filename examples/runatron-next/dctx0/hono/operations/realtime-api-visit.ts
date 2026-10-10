import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.RealtimeNavigation.LiveConversationDelivery.RealtimeApi.visit",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/liveconversationdelivery/http/realtime-api-visit.ts", export: "registerRealtimeApiVisit" },
  method: "post",
  path: "/api/realtime-api/visit",
  request: { parameters: { request: { in: "body" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { RealtimeAccessDenied: { status: 403, body: { from: "failure", optionEncoding: "tagged" } }, NavigationUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})


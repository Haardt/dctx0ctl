import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.RealtimeNavigation.LiveConversationDelivery.RealtimeApi.refresh",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/liveconversationdelivery/http/realtime-api-refresh.ts", export: "registerRealtimeApiRefresh" },
  method: "post",
  path: "/api/realtime-api/refresh",
  request: { parameters: { request: { in: "body" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { RealtimeAuthenticationRequired: { status: 401, body: { from: "failure", optionEncoding: "tagged" } }, RealtimeAccessDenied: { status: 403, body: { from: "failure", optionEncoding: "tagged" } }, NavigationUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})


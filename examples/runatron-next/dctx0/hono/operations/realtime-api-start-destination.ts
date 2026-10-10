import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.RealtimeNavigation.LiveConversationDelivery.RealtimeApi.startDestination",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/liveconversationdelivery/http/realtime-api-start-destination.ts", export: "registerRealtimeApiStartDestination" },
  method: "get",
  path: "/api/realtime-api/start-destination",
  request: { parameters: { personId: { in: "query" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "nullable" } },
    failures: { NavigationUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})


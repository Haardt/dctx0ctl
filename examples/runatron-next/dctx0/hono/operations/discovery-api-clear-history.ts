import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.Discovery.FindAndNavigate.DiscoveryApi.clearHistory",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/findandnavigate/http/discovery-api-clear-history.ts", export: "registerDiscoveryApiClearHistory" },
  method: "post",
  path: "/api/discovery-api/clear-history",
  request: { parameters: { personId: { in: "query" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { SearchAccessDenied: { status: 403, body: { from: "failure", optionEncoding: "tagged" } }, DiscoveryUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})


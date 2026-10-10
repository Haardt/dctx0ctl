import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.Discovery.FindAndNavigate.DiscoveryApi.recent",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/findandnavigate/http/discovery-api-recent.ts", export: "registerDiscoveryApiRecent" },
  method: "get",
  path: "/api/discovery-api/recent",
  request: { parameters: { personId: { in: "query" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { SearchAccessDenied: { status: 403, body: { from: "failure", optionEncoding: "tagged" } }, DiscoveryUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})


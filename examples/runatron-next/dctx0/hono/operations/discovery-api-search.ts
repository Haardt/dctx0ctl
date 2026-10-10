import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.Discovery.FindAndNavigate.DiscoveryApi.search",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/findandnavigate/http/discovery-api-search.ts", export: "registerDiscoveryApiSearch" },
  method: "post",
  path: "/api/discovery-api/search",
  request: { parameters: { request: { in: "body" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { SearchAccessDenied: { status: 403, body: { from: "failure", optionEncoding: "tagged" } }, SearchQueryRejected: { status: 409, body: { from: "failure", optionEncoding: "tagged" } }, DiscoveryUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})


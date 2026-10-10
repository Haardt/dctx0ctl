import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.Discovery.FindAndNavigate.DiscoveryApi.suggestPeople",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/findandnavigate/http/discovery-api-suggest-people.ts", export: "registerDiscoveryApiSuggestPeople" },
  method: "post",
  path: "/api/discovery-api/suggest-people",
  request: { parameters: { request: { in: "body" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { DirectoryAccessDenied: { status: 403, body: { from: "failure", optionEncoding: "tagged" } }, DiscoveryUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})


import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.Discovery.FindAndNavigate.DiscoveryApi.locate",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/findandnavigate/http/discovery-api-locate.ts", export: "registerDiscoveryApiLocate" },
  method: "post",
  path: "/api/discovery-api/locate",
  request: { parameters: { request: { in: "body" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { SearchTargetNotFound: { status: 404, body: { from: "failure", optionEncoding: "tagged" } }, SearchAccessDenied: { status: 403, body: { from: "failure", optionEncoding: "tagged" } }, DiscoveryUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})


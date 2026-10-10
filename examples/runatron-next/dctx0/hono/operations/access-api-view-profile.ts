import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.IdentityAccess.HumanAccess.AccessApi.viewProfile",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/humanaccess/http/access-api-view-profile.ts", export: "registerAccessApiViewProfile" },
  method: "post",
  path: "/api/access-api/view-profile",
  request: { parameters: { requesterId: { in: "query" }, personId: { in: "query" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { AccountUnavailable: { status: 404, body: { from: "failure", optionEncoding: "tagged" } }, AccountNotActive: { status: 400, body: { from: "failure", optionEncoding: "tagged" } }, AccessPersistenceUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})


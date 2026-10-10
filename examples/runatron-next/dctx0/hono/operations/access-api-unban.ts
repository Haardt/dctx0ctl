import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.IdentityAccess.HumanAccess.AccessApi.unban",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/humanaccess/http/access-api-unban.ts", export: "registerAccessApiUnban" },
  method: "post",
  path: "/api/access-api/unban",
  request: { parameters: { request: { in: "body" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { AccountUnavailable: { status: 404, body: { from: "failure", optionEncoding: "tagged" } }, AccessPersistenceUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})


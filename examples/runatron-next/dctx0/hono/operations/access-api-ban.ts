import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.IdentityAccess.HumanAccess.AccessApi.ban",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/humanaccess/http/access-api-ban.ts", export: "registerAccessApiBan" },
  method: "post",
  path: "/api/access-api/ban",
  request: { parameters: { request: { in: "body" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { AccountUnavailable: { status: 404, body: { from: "failure", optionEncoding: "tagged" } }, AccessPersistenceUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})


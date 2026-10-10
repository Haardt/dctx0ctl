import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.IdentityAccess.HumanAccess.AccessApi.signOut",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/humanaccess/http/access-api-sign-out.ts", export: "registerAccessApiSignOut" },
  method: "post",
  path: "/api/access-api/sign-out",
  request: { parameters: { request: { in: "body" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { SessionRejected: { status: 401, body: { from: "failure", optionEncoding: "tagged" } }, AccessPersistenceUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})


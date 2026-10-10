import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "Public" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.IdentityAccess.HumanAccess.AccessApi.join",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/humanaccess/http/access-api-join.ts", export: "registerAccessApiJoin" },
  method: "post",
  path: "/api/access-api/join",
  request: { parameters: { request: { in: "body" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { InvitationRejected: { status: 401, body: { from: "failure", optionEncoding: "tagged" } }, ProfileRejected: { status: 409, body: { from: "failure", optionEncoding: "tagged" } }, AccessPersistenceUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})


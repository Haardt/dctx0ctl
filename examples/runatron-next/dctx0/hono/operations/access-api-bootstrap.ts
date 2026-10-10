import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "Public" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.IdentityAccess.HumanAccess.AccessApi.bootstrap",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/humanaccess/http/access-api-bootstrap.ts", export: "registerAccessApiBootstrap" },
  method: "post",
  path: "/api/access-api/bootstrap",
  request: { parameters: { request: { in: "body" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { WorkspaceAlreadyInitialized: { status: 409, body: { from: "failure", optionEncoding: "tagged" } }, ProfileRejected: { status: 409, body: { from: "failure", optionEncoding: "tagged" } }, AccessPersistenceUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})


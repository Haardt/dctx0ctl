import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.Automation.BotsAndExternalContent.AutomationAdminApi.rotateKey",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/botsandexternalcontent/http/automation-admin-api-rotate-key.ts", export: "registerAutomationAdminApiRotateKey" },
  method: "post",
  path: "/api/automation-admin-api/rotate-key",
  request: { parameters: { request: { in: "body" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { AutomationAdministrationDenied: { status: 403, body: { from: "failure", optionEncoding: "tagged" } }, AutomationNotFound: { status: 404, body: { from: "failure", optionEncoding: "tagged" } }, AutomationPersistenceUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})


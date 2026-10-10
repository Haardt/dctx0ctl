import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.Automation.BotsAndExternalContent.AutomationAdminApi.deactivate",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/botsandexternalcontent/http/automation-admin-api-deactivate.ts", export: "registerAutomationAdminApiDeactivate" },
  method: "post",
  path: "/api/automation-admin-api/deactivate",
  request: { parameters: { administratorId: { in: "query" }, automationId: { in: "query" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { AutomationAdministrationDenied: { status: 403, body: { from: "failure", optionEncoding: "tagged" } }, AutomationNotFound: { status: 404, body: { from: "failure", optionEncoding: "tagged" } }, AutomationPersistenceUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})


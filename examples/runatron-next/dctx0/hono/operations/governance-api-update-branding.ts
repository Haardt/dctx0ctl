import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.Administration.WorkspaceGovernance.GovernanceApi.updateBranding",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/workspacegovernance/http/governance-api-update-branding.ts", export: "registerGovernanceApiUpdateBranding" },
  method: "post",
  path: "/api/governance-api/update-branding",
  request: { parameters: { request: { in: "body" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { WorkspaceNotFound: { status: 404, body: { from: "failure", optionEncoding: "tagged" } }, AdministrationDenied: { status: 403, body: { from: "failure", optionEncoding: "tagged" } }, BrandingRejected: { status: 409, body: { from: "failure", optionEncoding: "tagged" } }, AdministrationPersistenceUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})


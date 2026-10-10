import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.WorkEvidence.ArtifactLineage.TicketApi.create",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/artifactlineage/tickets/http/ticket-api-create.ts", export: "registerTicketApiCreate" },
  method: "post",
  path: "/api/artifacts/tickets/create",
  request: { parameters: { request: { in: "body" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { TicketAccessDenied: { status: 403, body: { from: "failure", optionEncoding: "tagged" } }, TicketDefinitionRejected: { status: 409, body: { from: "failure", optionEncoding: "tagged" } }, TicketPersistenceUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})


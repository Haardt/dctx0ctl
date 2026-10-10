import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.WorkEvidence.ArtifactLineage.TicketApi.markStale",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/artifactlineage/tickets/http/ticket-api-mark-stale.ts", export: "registerTicketApiMarkStale" },
  method: "post",
  path: "/api/artifacts/tickets/mark-stale",
  request: { parameters: { request: { in: "body" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { TicketNotFound: { status: 404, body: { from: "failure", optionEncoding: "tagged" } }, TicketAccessDenied: { status: 403, body: { from: "failure", optionEncoding: "tagged" } }, TicketDefinitionRejected: { status: 409, body: { from: "failure", optionEncoding: "tagged" } }, TicketPersistenceUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})


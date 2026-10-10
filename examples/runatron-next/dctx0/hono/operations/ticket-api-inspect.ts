import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  operation: "RunatronChat.WorkEvidence.ArtifactLineage.TicketApi.inspect",
  family: "chatApi",
  mode: "custom",
  handler: { root: "apps/backend/src/artifactlineage/tickets/http/ticket-api-inspect.ts", export: "registerTicketApiInspect" },
  method: "get",
  path: "/api/artifacts/tickets/inspect",
  request: { parameters: { ticketId: { in: "query" }, actorId: { in: "query" } } },
  responses: {
    success: { status: 200, body: { from: "result", optionEncoding: "tagged" } },
    failures: { TicketNotFound: { status: 404, body: { from: "failure", optionEncoding: "tagged" } }, TicketAccessDenied: { status: 403, body: { from: "failure", optionEncoding: "tagged" } }, TicketPersistenceUnavailable: { status: 503, body: { from: "failure", optionEncoding: "tagged" } } }
  }
})


export type RunatronHttpAuthorization = "Public" | "HumanSession" | "RunnerKey" | "RunnerChannelAuthority" | "AutomationKey"

export type RunatronHonoOperationBinding = Readonly<{
  operation: string
  family: string
  mode: "custom"
  handler: Readonly<{ root: string; export: string }>
  method: "delete" | "get" | "patch" | "post" | "put"
  path: string
  request: Readonly<{
    parameters: Readonly<Record<string, Readonly<{ in: "body" | "header" | "path" | "query"; name?: string }>>>
  }>
  responses: Readonly<{
    success: Readonly<{ status: number; body: Readonly<{ from: "result"; optionEncoding: "tagged" | "nullable" }> }>
    failures: Readonly<Record<string, Readonly<{
      status: number
      body: Readonly<{ from: "failure"; optionEncoding: "tagged" | "nullable" }>
    }>>>
  }>
}>

export type RunatronHonoBinding = Readonly<{
  version: 3
  surfaces: readonly string[]
  root: string
  factory: string
  entrypoint: Readonly<{ root: string; export: string }>
  operations: Readonly<{ root: string; include: string }>
  families: Readonly<Record<string, Readonly<{
    authentication: "none"
    requestTransport: "hono"
    failureMapping: "declared-status"
    responseEnvelope: "json"
  }>>>
}>

export function defineHonoOperationBinding<const Binding extends RunatronHonoOperationBinding>(binding: Binding): Binding {
  return binding
}

export function defineHonoBindings<const Binding extends RunatronHonoBinding>(binding: Binding): Binding {
  return binding
}

export type RunatronHttpOperationContract = RunatronHonoOperationBinding & { readonly authorization: RunatronHttpAuthorization; readonly responseContentTypes?: readonly ("application/json" | "text/event-stream")[] }

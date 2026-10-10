import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "RunnerChannelAuthority" as const

export default defineHonoOperationBinding({
  "operation": "RunatronChat.AgentExecution.InspectableRuns.AgentRunApi.publishSummary",
  "family": "chatApi",
  "mode": "custom",
  "handler": {
    "root": "apps/backend/src/inspectableruns/http/agent-run-api-publish-summary.ts",
    "export": "registerAgentRunApiPublishSummary"
  },
  "method": "post",
  "path": "/api/runner/runs/summaries/publish",
  "request": {
    "parameters": {
      "request": {
        "in": "body"
      }
    }
  },
  "responses": {
    "success": {
      "status": 200,
      "body": {
        "from": "result",
        "optionEncoding": "tagged"
      }
    },
    "failures": {
      "RunnerAuthenticationRejected": {
        "status": 401,
        "body": {
          "from": "failure",
          "optionEncoding": "tagged"
        }
      },
      "AgentRunNotFound": {
        "status": 404,
        "body": {
          "from": "failure",
          "optionEncoding": "tagged"
        }
      },
      "RunnerLeaseRejected": {
        "status": 409,
        "body": {
          "from": "failure",
          "optionEncoding": "tagged"
        }
      },
      "RunDelegationRejected": {
        "status": 403,
        "body": {
          "from": "failure",
          "optionEncoding": "tagged"
        }
      },
      "RunSummaryRequired": {
        "status": 422,
        "body": {
          "from": "failure",
          "optionEncoding": "tagged"
        }
      },
      "InvalidRunTransition": {
        "status": 409,
        "body": {
          "from": "failure",
          "optionEncoding": "tagged"
        }
      },
      "ChatPublicationUnavailable": {
        "status": 503,
        "body": {
          "from": "failure",
          "optionEncoding": "tagged"
        }
      },
      "RunLedgerUnavailable": {
        "status": 503,
        "body": {
          "from": "failure",
          "optionEncoding": "tagged"
        }
      }
    }
  }
}
)

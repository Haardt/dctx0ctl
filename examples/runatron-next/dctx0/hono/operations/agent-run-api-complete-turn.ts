import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "RunnerChannelAuthority" as const

export default defineHonoOperationBinding({
  "operation": "RunatronChat.AgentExecution.InspectableRuns.AgentRunApi.completeTurn",
  "family": "chatApi",
  "mode": "custom",
  "handler": {
    "root": "apps/backend/src/inspectableruns/http/agent-run-api-complete-turn.ts",
    "export": "registerAgentRunApiCompleteTurn"
  },
  "method": "post",
  "path": "/api/runner/runs/turns/complete",
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
      "RunHistoryRequired": {
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

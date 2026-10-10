import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "RunnerChannelAuthority" as const

export default defineHonoOperationBinding({
  "operation": "RunatronChat.AgentExecution.InspectableRuns.AgentRunApi.progress",
  "family": "chatApi",
  "mode": "custom",
  "handler": {
    "root": "apps/backend/src/inspectableruns/http/agent-run-api-progress.ts",
    "export": "registerAgentRunApiProgress"
  },
  "method": "post",
  "path": "/api/runner/runs/progress",
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
      "RunActivityConflict": {
        "status": 409,
        "body": {
          "from": "failure",
          "optionEncoding": "tagged"
        }
      },
      "RunBatchLimitExceeded": {
        "status": 413,
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
      "RunDelegationRejected": {
        "status": 403,
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

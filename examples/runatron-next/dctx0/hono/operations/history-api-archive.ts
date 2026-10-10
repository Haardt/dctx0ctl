import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "RunnerChannelAuthority" as const

export default defineHonoOperationBinding({
  "operation": "RunatronChat.PortableHistory.RunHistory.HistoryApi.archive",
  "family": "chatApi",
  "mode": "custom",
  "handler": {
    "root": "apps/backend/src/runhistory/http/history-api-archive.ts",
    "export": "registerHistoryApiArchive"
  },
  "method": "post",
  "path": "/api/runner/run-history/archive",
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
      "HistoryAuthenticationRejected": {
        "status": 401,
        "body": {
          "from": "failure",
          "optionEncoding": "tagged"
        }
      },
      "HistoryAccessDenied": {
        "status": 403,
        "body": {
          "from": "failure",
          "optionEncoding": "tagged"
        }
      },
      "HistoryConflict": {
        "status": 409,
        "body": {
          "from": "failure",
          "optionEncoding": "tagged"
        }
      },
      "HistoryLimitExceeded": {
        "status": 413,
        "body": {
          "from": "failure",
          "optionEncoding": "tagged"
        }
      },
      "HistoryIntegrityRejected": {
        "status": 409,
        "body": {
          "from": "failure",
          "optionEncoding": "tagged"
        }
      },
      "HistoryPersistenceUnavailable": {
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

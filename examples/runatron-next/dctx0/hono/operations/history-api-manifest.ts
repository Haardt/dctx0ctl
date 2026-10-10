import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  "operation": "RunatronChat.PortableHistory.RunHistory.HistoryApi.manifest",
  "family": "chatApi",
  "mode": "custom",
  "handler": {
    "root": "apps/backend/src/runhistory/http/history-api-manifest.ts",
    "export": "registerHistoryApiManifest"
  },
  "method": "get",
  "path": "/api/agent-runs/:runId/history",
  "request": {
    "parameters": {
      "sessionCredential": {
        "in": "header",
        "name": "authorization"
      },
      "runId": {
        "in": "path"
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
      "HistoryNotFound": {
        "status": 404,
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

import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "Public" as const

export default defineHonoOperationBinding({
  "operation": "RunatronChat.IdentityAccess.HumanAccess.AccessApi.verifySession",
  "family": "chatApi",
  "mode": "custom",
  "handler": {
    "root": "apps/backend/src/humanaccess/http/access-api-verify-session.ts",
    "export": "registerAccessApiVerifySession"
  },
  "method": "post",
  "path": "/api/access-api/verify-session",
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
      "CredentialsRejected": {
        "status": 401,
        "body": {
          "from": "failure",
          "optionEncoding": "tagged"
        }
      },
      "AccessPersistenceUnavailable": {
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

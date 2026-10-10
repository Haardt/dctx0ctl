import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "HumanSession" as const

export default defineHonoOperationBinding({
  "operation": "RunatronChat.ChannelCollaboration.ChannelWork.ChannelWorkApi.context",
  "family": "chatApi",
  "mode": "custom",
  "handler": {
    "root": "apps/backend/src/channelwork/http/channel-work-api-context.ts",
    "export": "registerChannelWorkApiContext"
  },
  "method": "get",
  "path": "/api/channel-work/channels/:channelId/context",
  "request": {
    "parameters": {
      "humanCredential": {
        "in": "header",
        "name": "authorization"
      },
      "channelId": {
        "in": "path"
      },
      "cursor": {
        "in": "query"
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
      "ChannelWorkAuthenticationRequired": {
        "status": 401,
        "body": {
          "from": "failure",
          "optionEncoding": "tagged"
        }
      },
      "ChannelWorkAccessDenied": {
        "status": 403,
        "body": {
          "from": "failure",
          "optionEncoding": "tagged"
        }
      },
      "ChannelWorkChannelNotFound": {
        "status": 404,
        "body": {
          "from": "failure",
          "optionEncoding": "tagged"
        }
      },
      "ChannelWorkConflict": {
        "status": 409,
        "body": {
          "from": "failure",
          "optionEncoding": "tagged"
        }
      },
      "ChannelWorkUnavailable": {
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

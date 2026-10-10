import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "RunnerChannelAuthority" as const
export const responseContentTypes = ["application/json", "text/event-stream"] as const

export default defineHonoOperationBinding({
  "operation": "RunatronChat.ChannelCollaboration.ChannelWork.ChannelWorkApi.pendingAgentReplies",
  "family": "chatApi",
  "mode": "custom",
  "handler": {
    "root": "apps/backend/src/channelwork/http/channel-work-api-pending-agent-replies.ts",
    "export": "registerChannelWorkApiPendingAgentReplies"
  },
  "method": "post",
  "path": "/api/runner/agent-requests/available",
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
      "ChannelWorkAuthenticationRequired": {
        "status": 401,
        "body": {
          "from": "failure",
          "optionEncoding": "tagged"
        }
      },
      "ChannelWorkDelegationRejected": {
        "status": 403,
        "body": {
          "from": "failure",
          "optionEncoding": "tagged"
        }
      },
      "ChannelWorkSessionNotFound": {
        "status": 404,
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
      "ChannelWorkUnavailable": {
        "status": 503,
        "body": {
          "from": "failure",
          "optionEncoding": "tagged"
        }
      }
    }
  }
})

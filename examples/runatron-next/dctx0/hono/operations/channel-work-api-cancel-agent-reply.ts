import { defineHonoOperationBinding } from "../runatronHonoBinding"

export const authorization = "RunnerChannelAuthority" as const

export default defineHonoOperationBinding({
  "operation": "RunatronChat.ChannelCollaboration.ChannelWork.ChannelWorkApi.cancelAgentReply",
  "family": "chatApi",
  "mode": "custom",
  "handler": {
    "root": "apps/backend/src/channelwork/http/channel-work-api-cancel-agent-reply.ts",
    "export": "registerChannelWorkApiCancelAgentReply"
  },
  "method": "post",
  "path": "/api/runner/agent-requests/cancel",
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
      "AgentReplyRequestNotFound": {
        "status": 404,
        "body": {
          "from": "failure",
          "optionEncoding": "tagged"
        }
      },
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

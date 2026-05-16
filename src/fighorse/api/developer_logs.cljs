(ns fighorse.api.developer-logs
  "Figma Developer Logs API — 1 endpoint.
   https://developers.figma.com/docs/rest-api/developer-logs-endpoints/"
  (:require [fighorse.utils.http :as http]))

(defn get-developer-logs
  "POST /v1/developer_logs
   Get developer logs for REST API and MCP server requests in an organization.
   Requires org admin scope."
  [token & {:keys [token-type token-value token-name user-email ip-address event-source date-range limit cursor]}]
  (http/post "/v1/developer_logs" token
             :body {:token_type token-type
                    :token token-value
                    :token_name token-name
                    :user_email user-email
                    :ip_address ip-address
                    :event_source event-source
                    :date_range date-range
                    :limit limit
                    :cursor cursor}))

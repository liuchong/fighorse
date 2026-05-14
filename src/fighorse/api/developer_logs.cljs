(ns fighorse.api.developer-logs
  "Figma Developer Logs API — 1 endpoint.
   https://developers.figma.com/docs/rest-api/developer-logs-endpoints/"
  (:require [fighorse.utils.http :as http]))

(defn get-developer-logs
  "GET /v1/developer_logs
   Get developer logs for REST API and MCP server requests in an organization.
   Requires org admin scope."
  [token & {:keys [after before start-time end-time limit cursor]}]
  (http/get "/v1/developer_logs" token
            :params {:after after
                     :before before
                     :start_time start-time
                     :end_time end-time
                     :limit limit
                     :cursor cursor}))

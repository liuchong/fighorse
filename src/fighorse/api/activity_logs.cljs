(ns fighorse.api.activity-logs
  "Figma Activity Logs API — 1 endpoint.
   https://developers.figma.com/docs/rest-api/activity-logs-endpoints/"
  (:require [fighorse.utils.http :as http]))

(defn get-activity-logs
  "GET /v1/activity_logs
   Get activity logs as an organization admin.
   Requires org admin scope."
  [token & {:keys [start-time end-time events limit order]}]
  (http/get "/v1/activity_logs" token
            :params {:start_time start-time
                     :end_time end-time
                     :events events
                     :limit limit
                     :order order}))

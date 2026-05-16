(ns fighorse.api.webhooks
  "Figma Webhooks API — 4 endpoints.
   https://developers.figma.com/docs/rest-api/webhooks-endpoints/"
  (:require [fighorse.utils.http :as http]))

(defn get-webhooks
  "GET /v2/webhooks
   Returns a list of webhooks corresponding to the context or plan provided."
  [token & {:keys [context plan after]}]
  (http/get "/v2/webhooks" token
            :params {:context context
                     :plan plan
                     :after after}))

(defn create-webhook
  "POST /v2/webhooks
   Create a new webhook."
  ([token body]
   (http/post "/v2/webhooks" token :body body))
  ([token event-type team-id endpoint & {:keys [passcode description status]}]
   (create-webhook token {:event_type event-type
                          :team_id team-id
                          :endpoint endpoint
                          :passcode passcode
                          :description description
                          :status status})))

(defn get-webhook
  "GET /v2/webhooks/{webhook_id}
   Get a webhook by id."
  [token webhook-id]
  (http/get (str "/v2/webhooks/" (http/path-segment webhook-id)) token))

(defn update-webhook
  "PUT /v2/webhooks/{webhook_id}
   Update a webhook by id."
  [token webhook-id body]
  (http/put (str "/v2/webhooks/" (http/path-segment webhook-id)) token
            :body body))

(defn delete-webhook
  "DELETE /v2/webhooks/{webhook_id}
   Delete a webhook by id."
  [token webhook-id]
  (http/delete (str "/v2/webhooks/" (http/path-segment webhook-id)) token))

(defn get-team-webhooks
  "GET /v2/teams/{team_id}/webhooks
   Deprecated official endpoint for team webhooks."
  [token team-id]
  (http/get (str "/v2/teams/" (http/path-segment team-id) "/webhooks") token))

(defn get-webhook-requests
  "GET /v2/webhooks/{webhook_id}/requests
   Get recent requests for a webhook."
  [token webhook-id & {:keys [cursor]}]
  (http/get (str "/v2/webhooks/" (http/path-segment webhook-id) "/requests") token
            :params {:cursor cursor}))

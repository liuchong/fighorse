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
  [token event-type team-id endpoint & {:keys [passcode description status]}]
  (http/post "/v2/webhooks" token
             :body {:event_type event-type
                    :team_id team-id
                    :endpoint endpoint
                    :passcode passcode
                    :description description
                    :status status}))

(defn get-webhook
  "GET /v2/webhooks/{webhook_id}
   Get a webhook by id."
  [token webhook-id]
  (http/get (str "/v2/webhooks/" (http/path-segment webhook-id)) token))

(defn delete-webhook
  "DELETE /v2/webhooks/{webhook_id}
   Delete a webhook by id."
  [token webhook-id]
  (http/delete (str "/v2/webhooks/" (http/path-segment webhook-id)) token))

(ns fighorse.api.payments
  "Figma Payments API — 2 endpoints.
   https://developers.figma.com/docs/rest-api/payments-endpoints/"
  (:require [fighorse.utils.http :as http]))

(defn get-payments
  "GET /v1/payments
   Get purchase information for your Community resources."
  [token & {:keys [resource-type resource-id plugin-id]}]
  (http/get "/v1/payments" token
            :params {:resource_type resource-type
                     :resource_id resource-id
                     :plugin_id plugin-id}))

(defn get-payment
  "GET /v1/payments/{resource_type}/{resource_id}
   Get purchase information for a specific resource."
  [token resource-type resource-id]
  (http/get (str "/v1/payments/" (http/path-segment resource-type) "/" (http/path-segment resource-id)) token))

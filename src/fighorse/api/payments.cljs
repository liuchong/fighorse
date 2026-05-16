(ns fighorse.api.payments
  "Figma Payments API — 2 endpoints.
   https://developers.figma.com/docs/rest-api/payments-endpoints/"
  (:require [fighorse.utils.http :as http]))

(defn get-payments
  "GET /v1/payments
   Get purchase information for your Community resources."
  [token & {:keys [plugin-payment-token user-id community-file-id plugin-id widget-id]}]
  (http/get "/v1/payments" token
            :params {:plugin_payment_token plugin-payment-token
                     :user_id user-id
                     :community_file_id community-file-id
                     :plugin_id plugin-id
                     :widget_id widget-id}))

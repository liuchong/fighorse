(ns fighorse.api.analytics
  "Figma Library Analytics API — 5 endpoints.
   https://developers.figma.com/docs/rest-api/library-analytics-endpoints/"
  (:require [fighorse.utils.http :as http]))

(defn get-library-analytics-component-usages
  "GET /v1/analytics/libraries/{file_key}/component/usages
   Get component usage analytics for a library."
  [token file-key & {:keys [cursor start-date end-date order-direction]}]
  (http/get (str "/v1/analytics/libraries/" (http/path-segment file-key) "/component/usages") token
            :params {:cursor cursor
                     :start_date start-date
                     :end_date end-date
                     :order_direction order-direction}))

(defn get-library-analytics-component-actions
  "GET /v1/analytics/libraries/{file_key}/component/actions
   Get component action analytics for a library."
  [token file-key & {:keys [cursor start-date end-date order-direction]}]
  (http/get (str "/v1/analytics/libraries/" (http/path-segment file-key) "/component/actions") token
            :params {:cursor cursor
                     :start_date start-date
                     :end_date end-date
                     :order_direction order-direction}))

(defn get-library-analytics-style-usages
  "GET /v1/analytics/libraries/{file_key}/style/usages
   Get style usage analytics for a library."
  [token file-key & {:keys [cursor start-date end-date order-direction]}]
  (http/get (str "/v1/analytics/libraries/" (http/path-segment file-key) "/style/usages") token
            :params {:cursor cursor
                     :start_date start-date
                     :end_date end-date
                     :order_direction order-direction}))

(defn get-library-analytics-variable-usages
  "GET /v1/analytics/libraries/{file_key}/variable/usages
   Get variable usage analytics for a library."
  [token file-key & {:keys [cursor start-date end-date order-direction]}]
  (http/get (str "/v1/analytics/libraries/" (http/path-segment file-key) "/variable/usages") token
            :params {:cursor cursor
                     :start_date start-date
                     :end_date end-date
                     :order_direction order-direction}))

(defn get-library-analytics-variable-actions
  "GET /v1/analytics/libraries/{file_key}/variable/actions
   Get variable action analytics for a library."
  [token file-key & {:keys [cursor start-date end-date order-direction]}]
  (http/get (str "/v1/analytics/libraries/" (http/path-segment file-key) "/variable/actions") token
            :params {:cursor cursor
                     :start_date start-date
                     :end_date end-date
                     :order_direction order-direction}))

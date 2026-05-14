(ns fighorse.api.oembed
  "Figma oEmbed API — 1 endpoint.
   https://developers.figma.com/docs/rest-api/oembed-endpoints/"
  (:require [fighorse.utils.http :as http]))

(defn get-oembed
  "GET /v1/oembed
   Get oEmbed data for Figma files and published Makes.
   Note: This endpoint does not require authentication."
  [& {:keys [url max-width max-height]}]
  (http/get "/v1/oembed" nil
            :params {:url url
                     :max_width max-width
                     :max_height max-height}))

(ns fighorse.api.users
  "Figma Users API — 1 endpoint.
   https://developers.figma.com/docs/rest-api/users-endpoints/"
  (:require [fighorse.utils.http :as http]))

(defn get-me
  "GET /v1/me
   Returns the user information for the currently authenticated user."
  [token]
  (http/get "/v1/me" token))

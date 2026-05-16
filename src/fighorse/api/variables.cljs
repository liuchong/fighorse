(ns fighorse.api.variables
  "Figma Variables API — current public REST endpoints.
   https://developers.figma.com/docs/rest-api/variables-endpoints/"
  (:require [fighorse.utils.http :as http]))

(defn get-local-variables
  "GET /v1/files/{file_key}/variables/local
   Get the local variables in a file."
  [token file-key]
  (http/get (str "/v1/files/" (http/path-segment file-key) "/variables/local") token))

(defn get-published-variables
  "GET /v1/files/{file_key}/variables/published
   Get the published variables in a file."
  [token file-key]
  (http/get (str "/v1/files/" (http/path-segment file-key) "/variables/published") token))

(defn post-variables
  "POST /v1/files/{file_key}/variables
   Bulk create, update, and delete variables and variable collections."
  [token file-key changes]
  (http/post (str "/v1/files/" (http/path-segment file-key) "/variables") token
             :body changes))

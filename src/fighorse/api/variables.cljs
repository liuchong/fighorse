(ns fighorse.api.variables
  "Figma Variables API — 4 endpoints.
   https://developers.figma.com/docs/rest-api/variables-endpoints/"
  (:require [fighorse.utils.http :as http]))

(defn get-local-variables
  "GET /v1/files/{file_key}/variables/local
   Get the local variables in a file."
  [token file-key]
  (http/get (str "/v1/files/" (http/path-segment file-key) "/variables/local") token))

(defn post-local-variables
  "POST /v1/files/{file_key}/variables/local
   Create or update local variables in a file."
  [token file-key variable-collection-changes]
  (http/post (str "/v1/files/" (http/path-segment file-key) "/variables/local") token
             :body {:variableCollectionChanges variable-collection-changes}))

(defn get-published-variables
  "GET /v1/files/{file_key}/variables/published
   Get the published variables in a file."
  [token file-key]
  (http/get (str "/v1/files/" (http/path-segment file-key) "/variables/published") token))

(defn post-published-variables
  "POST /v1/files/{file_key}/variables/published
   Publish variables from a file."
  [token file-key variable-collection-changes]
  (http/post (str "/v1/files/" (http/path-segment file-key) "/variables/published") token
             :body {:variableCollectionChanges variable-collection-changes}))

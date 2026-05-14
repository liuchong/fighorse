(ns fighorse.api.dev-resources
  "Figma Dev Resources API — 3 endpoints.
   https://developers.figma.com/docs/rest-api/dev-resources-endpoints/"
  (:require [fighorse.utils.http :as http]))

(defn get-dev-resources
  "GET /v1/files/{file_key}/dev_resources
   Get all dev resources in a file."
  [token file-key & {:keys [node-id]}]
  (http/get (str "/v1/files/" (http/path-segment file-key) "/dev_resources") token
            :params {:node_id node-id}))

(defn create-dev-resource
  "POST /v1/files/{file_key}/dev_resources
   Create a new dev resource in a file."
  [token file-key name url node-id]
  (http/post (str "/v1/files/" (http/path-segment file-key) "/dev_resources") token
             :body {:name name
                    :url url
                    :node_id node-id}))

(defn delete-dev-resource
  "DELETE /v1/files/{file_key}/dev_resources
   Delete a dev resource from a file."
  [token file-key dev-resource-id]
  (http/delete (str "/v1/files/" (http/path-segment file-key) "/dev_resources") token
               :params {:dev_resource_id dev-resource-id}))

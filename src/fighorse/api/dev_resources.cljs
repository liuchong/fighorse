(ns fighorse.api.dev-resources
  "Figma Dev Resources API.
   https://developers.figma.com/docs/rest-api/dev-resources-endpoints/"
  (:require [fighorse.utils.http :as http]))

(defn get-dev-resources
  "GET /v1/files/{file_key}/dev_resources
   Get all dev resources in a file."
  [token file-key & {:keys [node-ids]}]
  (http/get (str "/v1/files/" (http/path-segment file-key) "/dev_resources") token
            :params {:node_ids node-ids}))

(defn post-dev-resources
  "POST /v1/dev_resources
   Bulk create dev resources across files."
  [token dev-resources]
  (http/post "/v1/dev_resources" token
             :body {:dev_resources dev-resources}))

(defn put-dev-resources
  "PUT /v1/dev_resources
   Bulk update dev resources across files."
  [token dev-resources]
  (http/put "/v1/dev_resources" token
            :body {:dev_resources dev-resources}))

(defn delete-dev-resource
  "DELETE /v1/files/{file_key}/dev_resources
   Delete a dev resource from a file."
  [token file-key dev-resource-id]
  (http/delete (str "/v1/files/" (http/path-segment file-key)
                    "/dev_resources/"
                    (http/path-segment dev-resource-id))
               token))

(defn create-dev-resource
  "Product helper for creating one dev resource through the official bulk endpoint."
  [token file-key name url node-id]
  (post-dev-resources token [{:name name
                              :url url
                              :file_key file-key
                              :node_id node-id}]))

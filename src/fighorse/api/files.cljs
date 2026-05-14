(ns fighorse.api.files
  "Figma Files API — 6 endpoints.
   https://developers.figma.com/docs/rest-api/file-endpoints/"
  (:require [fighorse.utils.http :as http]))

(defn get-file
  "GET /v1/files/{file_key}
   Returns the document identified by file_key as a JSON object."
  [token file-key & {:keys [version ids depth geometry plugin-data branch-data]}]
  (http/get (str "/v1/files/" (http/path-segment file-key)) token
            :params {:version version
                     :ids ids
                     :depth depth
                     :geometry geometry
                     :plugin_data plugin-data
                     :branch_data branch-data}))

(defn get-file-nodes
  "GET /v1/files/{file_key}/nodes
   Returns the nodes referenced by ids as a JSON object."
  [token file-key ids & {:keys [version depth geometry plugin-data]}]
  (http/get (str "/v1/files/" (http/path-segment file-key) "/nodes") token
            :params {:ids (if (string? ids) ids (clojure.string/join "," ids))
                     :version version
                     :depth depth
                     :geometry geometry
                     :plugin_data plugin-data}))

(defn get-images
  "GET /v1/images/{file_key}
   Renders images from a file. Returns a map of node IDs to URLs."
  [token file-key ids & {:keys [version scale format
                                svg-outline-text svg-include-id svg-include-node-id
                                svg-simplify-stroke contents-only use-absolute-bounds]
                          :or {format "png" scale 2}}]
  (http/get (str "/v1/images/" (http/path-segment file-key)) token
            :params {:ids (if (string? ids) ids (clojure.string/join "," ids))
                     :version version
                     :scale scale
                     :format format
                     :svg_outline_text svg-outline-text
                     :svg_include_id svg-include-id
                     :svg_include_node_id svg-include-node-id
                     :svg_simplify_stroke svg-simplify-stroke
                     :contents_only contents-only
                     :use_absolute_bounds use-absolute-bounds}))

(defn get-image-fills
  "GET /v1/files/{file_key}/images
   Returns download links for all images present in image fills."
  [token file-key]
  (http/get (str "/v1/files/" (http/path-segment file-key) "/images") token))

(defn get-file-meta
  "GET /v1/files/{file_key}/meta
   Get file metadata."
  [token file-key]
  (http/get (str "/v1/files/" (http/path-segment file-key) "/meta") token))

(defn get-file-versions
  "GET /v1/files/{file_key}/versions
   Fetches the version history of a file."
  [token file-key & {:keys [page-size before after]}]
  (http/get (str "/v1/files/" (http/path-segment file-key) "/versions") token
            :params {:page_size page-size
                     :before before
                     :after after}))

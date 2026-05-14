(ns fighorse.api.components
  "Figma Components & Component Sets API — 6 endpoints.
   https://developers.figma.com/docs/rest-api/component-endpoints/"
  (:require [fighorse.utils.http :as http]))

;; --- Components ---

(defn get-team-components
  "GET /v1/teams/{team_id}/components
   Get a paginated list of published components within a team library."
  [token team-id & {:keys [page-size after before]}]
  (http/get (str "/v1/teams/" (http/path-segment team-id) "/components") token
            :params {:page_size page-size
                     :after after
                     :before before}))

(defn get-file-components
  "GET /v1/files/{file_key}/components
   Get a list of published components within a file library."
  [token file-key]
  (http/get (str "/v1/files/" (http/path-segment file-key) "/components") token))

(defn get-component
  "GET /v1/components/{key}
   Get metadata on a component by key."
  [token key]
  (http/get (str "/v1/components/" (http/path-segment key)) token))

;; --- Component Sets ---

(defn get-team-component-sets
  "GET /v1/teams/{team_id}/component_sets
   Get a paginated list of published component sets within a team library."
  [token team-id & {:keys [page-size after before]}]
  (http/get (str "/v1/teams/" (http/path-segment team-id) "/component_sets") token
            :params {:page_size page-size
                     :after after
                     :before before}))

(defn get-file-component-sets
  "GET /v1/files/{file_key}/component_sets
   Get a list of published component sets within a file library."
  [token file-key]
  (http/get (str "/v1/files/" (http/path-segment file-key) "/component_sets") token))

(defn get-component-set
  "GET /v1/component_sets/{key}
   Get metadata on a published component set by key."
  [token key]
  (http/get (str "/v1/component_sets/" (http/path-segment key)) token))

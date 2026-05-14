(ns fighorse.api.styles
  "Figma Styles API — 3 endpoints.
   https://developers.figma.com/docs/rest-api/component-endpoints/"
  (:require [fighorse.utils.http :as http]))

(defn get-team-styles
  "GET /v1/teams/{team_id}/styles
   Get a paginated list of published styles within a team library."
  [token team-id & {:keys [page-size after before]}]
  (http/get (str "/v1/teams/" (http/path-segment team-id) "/styles") token
            :params {:page_size page-size
                     :after after
                     :before before}))

(defn get-file-styles
  "GET /v1/files/{file_key}/styles
   Get a list of published styles within a file library."
  [token file-key]
  (http/get (str "/v1/files/" (http/path-segment file-key) "/styles") token))

(defn get-style
  "GET /v1/styles/{key}
   Get metadata on a style by key."
  [token key]
  (http/get (str "/v1/styles/" (http/path-segment key)) token))

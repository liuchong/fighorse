(ns fighorse.api.projects
  "Figma Projects API — 2 endpoints.
   https://developers.figma.com/docs/rest-api/projects-endpoints/"
  (:require [fighorse.utils.http :as http]))

(defn get-team-projects
  "GET /v1/teams/{team_id}/projects
   Get a list of all Projects within the specified team."
  [token team-id]
  (http/get (str "/v1/teams/" (http/path-segment team-id) "/projects") token))

(defn get-project-files
  "GET /v1/projects/{project_id}/files
   Get a list of all Files within the specified project."
  [token project-id & {:keys [branch-data]}]
  (http/get (str "/v1/projects/" (http/path-segment project-id) "/files") token
            :params {:branch_data branch-data}))

(ns fighorse.mcp.resources
  "MCP resources and prompts for AI clients that support them."
  (:require [fighorse.api.coverage :as coverage]
            [fighorse.discovery :as discovery]
            [fighorse.experience :as experience]
            [fighorse.mcp.registry :as registry]))

(defn list-resources []
  {:resources (registry/resources)})

(defn read-resource [uri]
  (let [payload (case uri
                  "fighorse://capabilities" (discovery/manifest)
                  "fighorse://coverage" (coverage/coverage-report)
                  "fighorse://workflow/design-replication" (discovery/workflow)
                  "fighorse://experience/summary" (experience/guidance :limit 8)
                  nil)]
    (if payload
      {:contents [{:uri uri
                   :mimeType "application/json"
                   :text (js/JSON.stringify (clj->js payload) nil 2)}]}
      (throw (js/Error. (str "Unknown fighorse resource: " uri))))))

(defn list-prompts []
  {:prompts (registry/prompts)})

(defn get-prompt [name arguments]
  (case name
    "fighorse_design_replication"
    {:description "Use fighorse to implement a Figma design with a visual feedback and learning loop."
     :messages [{:role "user"
                 :content {:type "text"
                           :text (str "Use fighorse for this Figma design: " (or (:figma_url arguments) "<figma-url>") "\n"
                                      "Target platform: " (or (:platform arguments) "ask me if missing") "\n"
                                      "Asset format: " (or (:asset_format arguments) "ask me if missing") "\n\n"
                                      "Required workflow: discover_fighorse, list_experiences, get_design_package, export assets with manifest when needed, implement, visually compare, then record reusable lessons.")}}]}
    "fighorse_api_coverage"
    {:description "Audit fighorse Figma REST API parity."
     :messages [{:role "user"
                 :content {:type "text"
                           :text "Call fighorse://coverage or figma-api coverage, then verify there are no missing or drifted public REST operations."}}]}
    (throw (js/Error. (str "Unknown fighorse prompt: " name)))))

(ns fighorse.mcp.registry
  "Generated-ish MCP registry for official Figma REST operations."
  (:require [fighorse.api.coverage :as coverage]))

(defn official-tool
  [op]
  {:name (coverage/mcp-tool-name (:operation_id op))
   :description (str "Official Figma REST API operation "
                     (:operation_id op)
                     ": "
                     (:method op)
                     " "
                     (:path op)
                     (when (:deprecated op) " (deprecated by Figma)."))
   :inputSchema {:type "object"
                 :properties {:params {:type "object"
                                        :description "Path and query parameters, using official OpenAPI names such as file_key, ids, webhook_id, cursor."}
                              :body {:type "object"
                                     :description "JSON request body for POST/PUT/DELETE operations when required."}
                              :ai_guidance {:type "boolean"
                                            :description "When true, return a fighorse envelope with AI next-step guidance."}}
                 :required ["params"]}})

(defn official-tools []
  (mapv official-tool coverage/official-operations))

(defn official-tool-name? [name]
  (boolean (coverage/operation-for-tool-name name)))

(defn operation-id-for-tool [name]
  (:operation_id (coverage/operation-for-tool-name name)))

(defn write-tool-name? [name]
  (if-let [op (coverage/operation-for-tool-name name)]
    (coverage/write-operation? op)
    false))

(defn coverage-resource []
  {:uri "fighorse://coverage"
   :name "Figma REST API coverage"
   :description "Machine-readable coverage report for the official Figma REST OpenAPI snapshot."
   :mimeType "application/json"})

(defn capability-resource []
  {:uri "fighorse://capabilities"
   :name "fighorse capabilities"
   :description "Machine-readable fighorse discovery and capability manifest."
   :mimeType "application/json"})

(defn workflow-resource []
  {:uri "fighorse://workflow/design-replication"
   :name "Design replication workflow"
   :description "Recommended fighorse workflow for AI-assisted Figma implementation."
   :mimeType "application/json"})

(defn experience-resource []
  {:uri "fighorse://experience/summary"
   :name "Experience summary"
   :description "Prompt-ready local learned experience summary."
   :mimeType "application/json"})

(defn resources []
  [(capability-resource)
   (coverage-resource)
   (workflow-resource)
   (experience-resource)])

(defn prompts []
  [{:name "fighorse_design_replication"
    :description "Use fighorse to inspect a Figma URL, export assets, implement, visually verify, and record reusable lessons."
    :arguments [{:name "figma_url" :description "Figma design/file/proto URL" :required true}
                {:name "platform" :description "Target platform/framework" :required false}
                {:name "asset_format" :description "Preferred asset format" :required false}]}
   {:name "fighorse_api_coverage"
    :description "Audit fighorse against the official Figma REST OpenAPI coverage report."
    :arguments []}])

(ns fighorse.api.coverage
  "Figma OpenAPI coverage registry.

  This registry is intentionally explicit. It is the local contract that keeps
  REST wrappers, CLI, MCP, discovery, and tests aligned with the vendored
  Figma OpenAPI snapshot."
  (:require [clojure.string :as str]))

(def openapi-version "0.38.0")

(defn- op
  [tag method path operation-id & {:keys [deprecated policy]}]
  {:tag tag
   :method method
   :path path
   :operation_id operation-id
   :deprecated (boolean deprecated)
   :policy (or policy (if (= "GET" method) "readonly" "write"))
   :status "covered"})

(def official-operations
  [(op "Files" "GET" "/v1/files/{file_key}" "getFile")
   (op "Files" "GET" "/v1/files/{file_key}/nodes" "getFileNodes")
   (op "Files" "GET" "/v1/images/{file_key}" "getImages")
   (op "Files" "GET" "/v1/files/{file_key}/images" "getImageFills")
   (op "Files" "GET" "/v1/files/{file_key}/meta" "getFileMeta")
   (op "Projects" "GET" "/v1/teams/{team_id}/projects" "getTeamProjects")
   (op "Projects" "GET" "/v1/projects/{project_id}/files" "getProjectFiles")
   (op "Files" "GET" "/v1/files/{file_key}/versions" "getFileVersions")
   (op "Comments" "GET" "/v1/files/{file_key}/comments" "getComments")
   (op "Comments" "POST" "/v1/files/{file_key}/comments" "postComment")
   (op "Comments" "DELETE" "/v1/files/{file_key}/comments/{comment_id}" "deleteComment")
   (op "Comment Reactions" "GET" "/v1/files/{file_key}/comments/{comment_id}/reactions" "getCommentReactions")
   (op "Comment Reactions" "POST" "/v1/files/{file_key}/comments/{comment_id}/reactions" "postCommentReaction")
   (op "Comment Reactions" "DELETE" "/v1/files/{file_key}/comments/{comment_id}/reactions" "deleteCommentReaction")
   (op "Users" "GET" "/v1/me" "getMe")
   (op "Components" "GET" "/v1/teams/{team_id}/components" "getTeamComponents")
   (op "Components" "GET" "/v1/files/{file_key}/components" "getFileComponents")
   (op "Components" "GET" "/v1/components/{key}" "getComponent")
   (op "Component Sets" "GET" "/v1/teams/{team_id}/component_sets" "getTeamComponentSets")
   (op "Component Sets" "GET" "/v1/files/{file_key}/component_sets" "getFileComponentSets")
   (op "Component Sets" "GET" "/v1/component_sets/{key}" "getComponentSet")
   (op "Styles" "GET" "/v1/teams/{team_id}/styles" "getTeamStyles")
   (op "Styles" "GET" "/v1/files/{file_key}/styles" "getFileStyles")
   (op "Styles" "GET" "/v1/styles/{key}" "getStyle")
   (op "Webhooks" "GET" "/v2/webhooks" "getWebhooks")
   (op "Webhooks" "POST" "/v2/webhooks" "postWebhook")
   (op "Webhooks" "GET" "/v2/webhooks/{webhook_id}" "getWebhook")
   (op "Webhooks" "PUT" "/v2/webhooks/{webhook_id}" "putWebhook")
   (op "Webhooks" "DELETE" "/v2/webhooks/{webhook_id}" "deleteWebhook")
   (op "Webhooks" "GET" "/v2/teams/{team_id}/webhooks" "getTeamWebhooks" :deprecated true)
   (op "Webhooks" "GET" "/v2/webhooks/{webhook_id}/requests" "getWebhookRequests")
   (op "Activity Logs" "GET" "/v1/activity_logs" "getActivityLogs")
   (op "Developer Logs" "POST" "/v1/developer_logs" "getDeveloperLogs" :policy "readonly")
   (op "Payments" "GET" "/v1/payments" "getPayments")
   (op "Variables" "GET" "/v1/files/{file_key}/variables/local" "getLocalVariables")
   (op "Variables" "GET" "/v1/files/{file_key}/variables/published" "getPublishedVariables")
   (op "Variables" "POST" "/v1/files/{file_key}/variables" "postVariables")
   (op "Dev Resources" "GET" "/v1/files/{file_key}/dev_resources" "getDevResources")
   (op "Dev Resources" "POST" "/v1/dev_resources" "postDevResources")
   (op "Dev Resources" "PUT" "/v1/dev_resources" "putDevResources")
   (op "Dev Resources" "DELETE" "/v1/files/{file_key}/dev_resources/{dev_resource_id}" "deleteDevResource")
   (op "Library Analytics" "GET" "/v1/analytics/libraries/{file_key}/component/actions" "getLibraryAnalyticsComponentActions")
   (op "Library Analytics" "GET" "/v1/analytics/libraries/{file_key}/component/usages" "getLibraryAnalyticsComponentUsages")
   (op "Library Analytics" "GET" "/v1/analytics/libraries/{file_key}/style/actions" "getLibraryAnalyticsStyleActions")
   (op "Library Analytics" "GET" "/v1/analytics/libraries/{file_key}/style/usages" "getLibraryAnalyticsStyleUsages")
   (op "Library Analytics" "GET" "/v1/analytics/libraries/{file_key}/variable/actions" "getLibraryAnalyticsVariableActions")
   (op "Library Analytics" "GET" "/v1/analytics/libraries/{file_key}/variable/usages" "getLibraryAnalyticsVariableUsages")
   (op "oEmbed" "GET" "/v1/oembed" "getOembed")])

(def official-mcp-only-capabilities
  [{:capability "native_canvas_write"
    :status "unsupported-by-public-rest-api"
    :reason "Figma MCP use_figma-style native canvas mutations are not exposed by the public REST OpenAPI."}
   {:capability "code_to_canvas"
    :status "unsupported-by-public-rest-api"
    :reason "generate_figma_design is an official MCP product capability, not a public REST endpoint."}
   {:capability "code_connect_auto_mapping"
    :status "unsupported-by-public-rest-api"
    :reason "Automatic Code Connect mapping discovery is tied to Figma's product integration."}
   {:capability "make_resources"
    :status "unsupported-by-public-rest-api"
    :reason "Make resources are exposed through official MCP product surfaces, not this REST snapshot."}
   {:capability "figjam_generation"
    :status "unsupported-by-public-rest-api"
    :reason "FigJam generation is not present in the public REST OpenAPI snapshot."}])

(def operations-by-id
  (into {} (map (juxt :operation_id identity) official-operations)))

(defn write-operation? [op]
  (= "write" (:policy op)))

(defn camel->snake [value]
  (-> (str value)
      (str/replace #"([a-z0-9])([A-Z])" "$1_$2")
      (str/replace #"-" "_")
      str/lower-case))

(defn mcp-tool-name [operation-id]
  (str "figma_" (camel->snake operation-id)))

(defn operation-for-tool-name [tool-name]
  (when (str/starts-with? tool-name "figma_")
    (let [needle (subs tool-name 6)]
      (first (filter #(= needle (camel->snake (:operation_id %))) official-operations)))))

(defn operation-summary []
  {:openapi_version openapi-version
   :operation_count (count official-operations)
   :covered_count (count (filter #(= "covered" (:status %)) official-operations))
   :write_count (count (filter write-operation? official-operations))
   :deprecated_count (count (filter :deprecated official-operations))
   :by_tag (into (sorted-map)
                 (map (fn [[tag ops]]
                        [tag {:total (count ops)
                              :write (count (filter write-operation? ops))
                              :deprecated (count (filter :deprecated ops))}]))
                 (group-by :tag official-operations))})

(defn coverage-report []
  {:kind "fighorse.api-coverage.v1"
   :source {:name "Figma REST OpenAPI"
            :version openapi-version
            :url "https://github.com/figma/rest-api-spec/blob/main/openapi/openapi.yaml"}
   :summary (operation-summary)
   :operations (mapv (fn [op]
                       (assoc op
                              :mcp_tool (mcp-tool-name (:operation_id op))
                              :cli_command (str "fighorse figma api " (:operation_id op) " --params '{...}'")
                              :policy (:policy op)))
                     official-operations)
   :official_mcp_only official-mcp-only-capabilities
   :ai_guidance {:use "Use this report to verify REST parity before relying on low-level Figma tools."
                 :next_step "If any operation is not covered, update fighorse.api operations, MCP tools, CLI dispatch, discovery, and tests together."}})

(defn coverage-report->markdown [report]
  (str "# fighorse Figma REST API Coverage\n\n"
       "- OpenAPI version: `" (get-in report [:source :version]) "`\n"
       "- Operations: `" (get-in report [:summary :operation_count]) "`\n"
       "- Covered: `" (get-in report [:summary :covered_count]) "`\n"
       "- Write operations: `" (get-in report [:summary :write_count]) "`\n\n"
       "## Operations\n\n"
       (str/join
        "\n"
        (map (fn [{:keys [method path operation_id tag deprecated status mcp_tool]}]
               (str "- `" method " " path "` `" operation_id "` "
                    "[" tag "] status=`" status "` mcp=`" mcp_tool "`"
                    (when deprecated " deprecated=true")))
             (:operations report)))
       "\n\n## Official MCP Product-Only Capabilities\n\n"
       (str/join
        "\n"
        (map (fn [{:keys [capability status reason]}]
               (str "- `" capability "` status=`" status "`: " reason))
             (:official_mcp_only report)))))

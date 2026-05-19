(ns fighorse.mcp.tools
  "MCP Tools registry for Figma API.
   Maps all Figma API endpoints + AI enhancements to MCP tools."
  (:require [clojure.string :as str]
            [fighorse.config :as config]
            [fighorse.api.operations :as operations]
            [fighorse.api.files :as files-api]
            [fighorse.api.projects :as projects-api]
            [fighorse.api.users :as users-api]
            [fighorse.api.components :as components-api]
            [fighorse.api.styles :as styles-api]
            [fighorse.api.comments :as comments-api]
            [fighorse.api.webhooks :as webhooks-api]
            [fighorse.api.variables :as variables-api]
            [fighorse.api.dev-resources :as dev-resources-api]
            [fighorse.api.activity-logs :as activity-logs-api]
            [fighorse.api.developer-logs :as developer-logs-api]
            [fighorse.api.analytics :as analytics-api]
            [fighorse.api.payments :as payments-api]
            [fighorse.api.oembed :as oembed-api]
            [fighorse.compact :as compact]
            [fighorse.design-package :as design-package]
            [fighorse.discovery :as discovery]
            [fighorse.experience :as experience]
            [fighorse.export.images :as img-export]
            [fighorse.figma :as figma]
            [fighorse.mcp.policy :as policy]
            [fighorse.mcp.registry :as registry]
            [fighorse.product.playbook :as playbook]
            [fighorse.product.visual-audit :as visual-audit]
            [fighorse.tokens :as tokens]
            [fighorse.utils.url :as figma-url]))

;; --- Tool definitions (JSON Schema) ---

(def ^:private all-tools
  [
   ;; === Files (official + enhanced) ===
   {:name "get_file"
    :description "Get a Figma file as JSON. Returns the full document tree, components, and styles."
    :inputSchema {:type "object"
                  :properties {:file_key {:type "string" :description "Figma file key"}
                               :depth {:type "number" :description "How deep into the document tree to traverse (1-5)"}
                               :version {:type "string" :description "Specific version ID to retrieve"}}
                  :required ["file_key"]}}

   {:name "get_file_nodes"
    :description "Get specific nodes from a Figma file. Useful for targeted queries."
    :inputSchema {:type "object"
                  :properties {:file_key {:type "string" :description "Figma file key"}
                               :node_ids {:type "string" :description "Comma-separated node IDs"}
                               :depth {:type "number" :description "Depth to traverse from each node"}}
                  :required ["file_key" "node_ids"]}}

   {:name "get_file_compact"
    :description "Get a Figma file with intelligent truncation for AI consumption. Removes low-value nodes to fit within a token budget."
    :inputSchema {:type "object"
                  :properties {:file_key {:type "string" :description "Figma file key"}
                               :max_tokens {:type "number" :description "Maximum token budget (approximate)"}
                               :depth {:type "number" :description "How deep into the tree to traverse"}}
                  :required ["file_key"]}}

   {:name "get_file_tree"
    :description "Get a lightweight tree view of a Figma file — only structure, names, types, and dimensions."
    :inputSchema {:type "object"
                  :properties {:file_key {:type "string" :description "Figma file key"}
                               :depth {:type "number" :description "How deep into the tree to traverse"}}
                  :required ["file_key"]}}

   {:name "get_file_meta"
    :description "Get metadata for a Figma file (name, last modified, thumbnail, etc)."
    :inputSchema {:type "object"
                  :properties {:file_key {:type "string" :description "Figma file key"}}
                  :required ["file_key"]}}

   {:name "get_file_versions"
    :description "Get version history of a Figma file."
    :inputSchema {:type "object"
                  :properties {:file_key {:type "string" :description "Figma file key"}
                               :page_size {:type "number" :description "Number of versions per page (max 50)"}}
                  :required ["file_key"]}}

   {:name "get_images"
    :description "Render images of nodes in a Figma file. Returns URLs that expire after 30 days."
    :inputSchema {:type "object"
                  :properties {:file_key {:type "string" :description "Figma file key"}
                               :node_ids {:type "string" :description "Comma-separated node IDs to render"}
                               :format {:type "string" :description "Image format: png, jpg, svg, pdf" :enum ["png" "jpg" "svg" "pdf"]}
                               :scale {:type "number" :description "Image scaling factor (0.01-4)"}}
                  :required ["file_key" "node_ids"]}}

   {:name "get_image_fills"
    :description "Get download links for all images in a file's image fills."
    :inputSchema {:type "object"
                  :properties {:file_key {:type "string" :description "Figma file key"}}
                  :required ["file_key"]}}

   ;; === Projects ===
   {:name "get_team_projects"
    :description "Get all projects within a Figma team."
    :inputSchema {:type "object"
                  :properties {:team_id {:type "string" :description "Figma team ID"}}
                  :required ["team_id"]}}

   {:name "get_project_files"
    :description "Get all files within a Figma project."
    :inputSchema {:type "object"
                  :properties {:project_id {:type "string" :description "Figma project ID"}}
                  :required ["project_id"]}}

   ;; === Users ===
   {:name "get_me"
    :description "Get information about the currently authenticated user."
    :inputSchema {:type "object" :properties {}}}

   ;; === Components ===
   {:name "get_team_components"
    :description "Get published components in a team library."
    :inputSchema {:type "object"
                  :properties {:team_id {:type "string" :description "Figma team ID"}
                               :page_size {:type "number" :description "Items per page (default 30)"}}
                  :required ["team_id"]}}

   {:name "get_file_components"
    :description "Get published components within a file library."
    :inputSchema {:type "object"
                  :properties {:file_key {:type "string" :description "Figma file key"}}
                  :required ["file_key"]}}

   {:name "get_component"
    :description "Get metadata for a specific component by key."
    :inputSchema {:type "object"
                  :properties {:component_key {:type "string" :description "Component key"}}
                  :required ["component_key"]}}

   {:name "get_file_component_sets"
    :description "Get published component sets within a file library."
    :inputSchema {:type "object"
                  :properties {:file_key {:type "string" :description "Figma file key"}}
                  :required ["file_key"]}}

   ;; === Styles ===
   {:name "get_team_styles"
    :description "Get published styles in a team library."
    :inputSchema {:type "object"
                  :properties {:team_id {:type "string" :description "Figma team ID"}
                               :page_size {:type "number" :description "Items per page (default 30)"}}
                  :required ["team_id"]}}

   {:name "get_file_styles"
    :description "Get published styles within a file library."
    :inputSchema {:type "object"
                  :properties {:file_key {:type "string" :description "Figma file key"}}
                  :required ["file_key"]}}

   {:name "get_style"
    :description "Get metadata for a specific style by key."
    :inputSchema {:type "object"
                  :properties {:style_key {:type "string" :description "Style key"}}
                  :required ["style_key"]}}

   ;; === Comments ===
   {:name "get_comments"
    :description "Get all comments on a Figma file."
    :inputSchema {:type "object"
                  :properties {:file_key {:type "string" :description "Figma file key"}
                               :as_md {:type "boolean" :description "Return comments as markdown"}}
                  :required ["file_key"]}}

   ;; === Variables ===
   {:name "get_local_variables"
    :description "Get local variables in a Figma file."
    :inputSchema {:type "object"
                  :properties {:file_key {:type "string" :description "Figma file key"}}
                  :required ["file_key"]}}

   {:name "get_published_variables"
    :description "Get published variables in a Figma file."
    :inputSchema {:type "object"
                  :properties {:file_key {:type "string" :description "Figma file key"}}
                  :required ["file_key"]}}

   ;; === Dev Resources ===
   {:name "get_dev_resources"
    :description "Get all dev resources (links) in a Figma file."
    :inputSchema {:type "object"
                  :properties {:file_key {:type "string" :description "Figma file key"}}
                  :required ["file_key"]}}

   ;; === Webhooks ===
   {:name "get_webhooks"
    :description "Get webhooks for the authenticated user's accessible contexts."
    :inputSchema {:type "object"
                  :properties {:context {:type "string" :description "Filter by context (team_id or file_key)"}}
                  :required []}}
   ])

(def ^:private extra-tools
  [{:name "discover_fighorse"
    :description "Call this first. Returns fighorse's self-description, Figma replication workflow, input/output contracts, and recommended tool order."
    :inputSchema {:type "object" :properties {} :required []}}
   {:name "check_fighorse_ready"
    :description "Check whether fighorse is ready to call Figma APIs, including Bun runtime and token presence. Does not call Figma. If auth.has_token is false, tell the user to run `fighorse auth login --token <FIGMA_TOKEN>` or set FIGMA_TOKEN before calling Figma API tools."
    :inputSchema {:type "object" :properties {} :required []}}
   {:name "parse_figma_url"
    :description "Parse a pasted Figma URL into file_key and node_id. Use before lower-level tools when the user gives a Figma URL."
    :inputSchema {:type "object"
                  :properties {:figma_url {:type "string" :description "Figma design/file/proto/board URL or raw file key"}}
                  :required ["figma_url"]}}
   {:name "get_replicate_workflow"
    :description "Return the self-learning workflow for recreating Figma designs with fighorse."
    :inputSchema {:type "object" :properties {} :required []}}
   {:name "get_experience_schema"
    :description "Return the versioned local experience schema. Call this before recording reusable Figma replication lessons."
    :inputSchema {:type "object" :properties {} :required []}}
   {:name "list_experiences"
    :description "Return prompt-ready local lessons relevant to a target platform, asset format, category, or tag. Call before implementation and after mismatches."
    :inputSchema {:type "object"
                  :properties {:platform {:type "string" :description "Target platform/framework, e.g. android-compose, ios-swiftui, web-react, flutter"}
                               :asset_format {:type "string" :description "Preferred asset format, e.g. png/svg/pdf/webp"}
                               :category {:type "string" :description "layout|typography|asset-export|platform|workflow|debugging|mcp|cli|other"}
                               :tag {:type "string" :description "Optional tag filter"}
                               :scope {:type "string" :description "Experience scope: auto, global, project, or merged"}
                               :project_dir {:type "string" :description "Project directory for project-scoped experience"}
                               :limit {:type "number" :description "Maximum records to return. Default 6."}}
                  :required []}}
   {:name "record_experience"
    :description "Append a reusable local lesson from a Figma replication run. This does not mutate Figma; it teaches future fighorse calls."
    :inputSchema {:type "object"
                  :properties {:summary {:type "string" :description "Short problem or insight title"}
                               :lesson {:type "string" :description "Reusable lesson learned"}
                               :category {:type "string" :description "layout|typography|asset-export|platform|workflow|debugging|mcp|cli|other"}
                               :severity {:type "string" :description "info|warning|critical"}
                               :platform {:type "string" :description "Target platform/framework, e.g. android-compose, ios-swiftui, web-react, flutter"}
                               :asset_format {:type "string" :description "Preferred export format, e.g. png/svg/pdf/webp"}
                               :figma_url {:type "string" :description "Optional Figma URL"}
                               :file_key {:type "string" :description "Optional Figma file key"}
                               :node_id {:type "string" :description "Optional Figma node id"}
                               :evidence {:type "string" :description "What happened: screenshot diff, build error, overlap, etc."}
                               :recommendation {:type "string" :description "Action AI should take next time"}
                               :tags {:type "array" :items {:type "string"} :description "Optional tags"}
                               :scope {:type "string" :description "Experience write scope: auto, global, project, or merged"}
                               :project_dir {:type "string" :description "Project directory for project-scoped experience"}
                               :client {:type "string" :description "AI client, e.g. cursor, codex, kimi-cli, opencode"}
                               :command {:type "string" :description "CLI command or MCP tool related to the lesson"}}
                  :required ["summary" "lesson"]}}
   {:name "get_design_package"
    :description "Best default tool for AI design replication. Accepts a Figma URL and returns compact structure, tokens, screenshots, assets, and implementation hints."
    :inputSchema {:type "object"
                  :properties {:figma_url {:type "string" :description "Pasted Figma URL. Preferred input."}
                               :file_key {:type "string" :description "Raw Figma file key when no URL is available."}
                               :node_id {:type "string" :description "Optional Figma node ID in API form, e.g. 376:5412. URL node-id is parsed automatically."}
                               :depth {:type "number" :description "Tree depth to fetch. Default 2."}
                               :max_tokens {:type "number" :description "Approximate token budget for compact context. Default 8000."}
                               :include_screenshot {:type "boolean" :description "Render screenshots. Default true."}
                               :include_assets {:type "boolean" :description "Include image fill asset URLs. Default false."}
                               :screenshot_format {:type "string" :enum ["png" "jpg" "svg" "pdf"]}
                               :scale {:type "number" :description "Screenshot scale. Default 2."}
                               :platform {:type "string" :description "Target platform/framework, e.g. android-compose, ios-swiftui, web-react, flutter. Ask the developer if unknown."}
                               :asset_format {:type "string" :description "Preferred export format for local slices/assets, e.g. png, svg, jpg, pdf, webp."}}
                  :required []}}
   {:name "visual_audit"
    :description "Build a structured visual audit checklist for a Figma URL and optional implementation screenshot."
    :inputSchema {:type "object"
                  :properties {:figma_url {:type "string"}
                               :screenshot_path {:type "string"}
                               :platform {:type "string"}
                               :asset_format {:type "string"}
                               :notes {:type "string"}}
                  :required []}}
   {:name "get_project_playbook"
    :description "Return a project-level fighorse implementation playbook assembled from guidance, API coverage, and local experience."
    :inputSchema {:type "object"
                  :properties {:platform {:type "string"}
                               :asset_format {:type "string"}
                               :project_dir {:type "string"}}
                  :required []}}
   {:name "get_design_context"
    :description "Get compact AI-oriented design context for a Figma file."
    :inputSchema {:type "object"
                  :properties {:file_key {:type "string"}
                               :depth {:type "number"}
                               :max_tokens {:type "number"}}
                  :required ["file_key"]}}
   {:name "get_node"
    :description "Get a specific node from a Figma file."
    :inputSchema {:type "object"
                  :properties {:file_key {:type "string"}
                               :node_id {:type "string"}
                               :depth {:type "number"}}
                  :required ["file_key" "node_id"]}}
   {:name "get_tokens"
    :description "Extract design tokens from a Figma file."
    :inputSchema {:type "object"
                  :properties {:file_key {:type "string"}
                               :depth {:type "number"}}
                  :required ["file_key"]}}
   {:name "get_screenshot"
    :description "Render a node screenshot. Alias of get_images for one or more node IDs."
    :inputSchema {:type "object"
                  :properties {:file_key {:type "string"}
                               :node_ids {:type "string"}
                               :format {:type "string" :enum ["png" "jpg" "svg" "pdf"]}
                               :scale {:type "number"}}
                  :required ["file_key" "node_ids"]}}
   {:name "export_images"
    :description "Download rendered node slices to local files. Uses safe filenames and can write manifest.json for AI/build-tool discovery."
    :inputSchema {:type "object"
                  :properties {:file_key {:type "string"}
                               :node_ids {:type "string" :description "Comma-separated node IDs to render"}
                               :format {:type "string" :enum ["png" "jpg" "svg" "pdf"]}
                               :scale {:type "number"}
                               :dest_dir {:type "string" :description "Output directory. Default ./.fighorse/exports. Must stay under ./.fighorse/exports, ./assets/fighorse, or ~/.fighorse/exports."}
                               :prefix {:type "string" :description "Optional filename prefix"}
                               :manifest {:type "boolean" :description "Write manifest.json in dest_dir"}}
                  :required ["file_key" "node_ids"]}}
   {:name "export_component"
    :description "Download a Figma component/control node as an image slice. Alias of export_images."
    :inputSchema {:type "object"
                  :properties {:file_key {:type "string"}
                               :node_ids {:type "string" :description "Comma-separated component/control node IDs"}
                               :format {:type "string" :enum ["png" "jpg" "svg" "pdf"]}
                               :scale {:type "number"}
                               :dest_dir {:type "string" :description "Output directory. Default ./.fighorse/exports. Must stay under ./.fighorse/exports, ./assets/fighorse, or ~/.fighorse/exports."}
                               :prefix {:type "string" :description "Optional filename prefix"}
                               :manifest {:type "boolean" :description "Write manifest.json in dest_dir"}}
                  :required ["file_key" "node_ids"]}}
   {:name "download_image_fills"
    :description "Download all image fills in a Figma file to local files. Adds file extensions from content type and can write manifest.json."
    :inputSchema {:type "object"
                  :properties {:file_key {:type "string"}
                               :dest_dir {:type "string" :description "Output directory. Default ./.fighorse/exports. Must stay under ./.fighorse/exports, ./assets/fighorse, or ~/.fighorse/exports."}
                               :prefix {:type "string" :description "Optional filename prefix"}
                               :manifest {:type "boolean" :description "Write manifest.json in dest_dir"}}
                  :required ["file_key"]}}
   {:name "get_comment_reactions" :description "Get reactions for a comment." :inputSchema {:type "object" :properties {:file_key {:type "string"} :comment_id {:type "string"} :cursor {:type "string"}} :required ["file_key" "comment_id"]}}
   {:name "get_team_component_sets" :description "Get published component sets in a team." :inputSchema {:type "object" :properties {:team_id {:type "string"} :page_size {:type "number"}} :required ["team_id"]}}
   {:name "get_component_set" :description "Get a component set by key." :inputSchema {:type "object" :properties {:component_set_key {:type "string"}} :required ["component_set_key"]}}
   {:name "get_webhook" :description "Get a webhook by ID." :inputSchema {:type "object" :properties {:webhook_id {:type "string"}} :required ["webhook_id"]}}
   {:name "get_team_webhooks" :description "Get webhooks for a team. Deprecated by Figma but kept for REST parity." :inputSchema {:type "object" :properties {:team_id {:type "string"}} :required ["team_id"]}}
   {:name "get_webhook_requests" :description "Get recent requests for a webhook." :inputSchema {:type "object" :properties {:webhook_id {:type "string"} :cursor {:type "string"}} :required ["webhook_id"]}}
   {:name "get_activity_logs" :description "Get organization activity logs." :inputSchema {:type "object" :properties {:limit {:type "number"} :cursor {:type "string"}} :required []}}
   {:name "get_developer_logs" :description "Get organization developer logs." :inputSchema {:type "object" :properties {:limit {:type "number"} :cursor {:type "string"}} :required []}}
   {:name "get_payments" :description "Get payments." :inputSchema {:type "object" :properties {:plugin_payment_token {:type "string"} :user_id {:type "string"} :community_file_id {:type "string"} :plugin_id {:type "string"} :widget_id {:type "string"}} :required []}}
   {:name "get_oembed" :description "Get Figma oEmbed data." :inputSchema {:type "object" :properties {:url {:type "string"} :max_width {:type "number"} :max_height {:type "number"}} :required ["url"]}}
   {:name "get_library_analytics_component_usages" :description "Get component usage analytics." :inputSchema {:type "object" :properties {:file_key {:type "string"}} :required ["file_key"]}}
   {:name "get_library_analytics_component_actions" :description "Get component action analytics." :inputSchema {:type "object" :properties {:file_key {:type "string"}} :required ["file_key"]}}
   {:name "get_library_analytics_style_actions" :description "Get style action analytics." :inputSchema {:type "object" :properties {:file_key {:type "string"} :group_by {:type "string"}} :required ["file_key"]}}
   {:name "get_library_analytics_style_usages" :description "Get style usage analytics." :inputSchema {:type "object" :properties {:file_key {:type "string"}} :required ["file_key"]}}
   {:name "get_library_analytics_variable_usages" :description "Get variable usage analytics." :inputSchema {:type "object" :properties {:file_key {:type "string"}} :required ["file_key"]}}
   {:name "get_library_analytics_variable_actions" :description "Get variable action analytics." :inputSchema {:type "object" :properties {:file_key {:type "string"}} :required ["file_key"]}}])

(def ^:private write-tools
  [{:name "post_comment"
    :description "WRITE MODE ONLY. Add a comment to a Figma file."
    :inputSchema {:type "object"
                  :properties {:file_key {:type "string" :description "Figma file key"}
                               :message {:type "string" :description "Comment text"}
                               :reply_to {:type "string" :description "Parent comment ID for replies"}}
                  :required ["file_key" "message"]}}
   {:name "delete_comment" :description "WRITE MODE ONLY. Delete a Figma comment." :inputSchema {:type "object" :properties {:file_key {:type "string"} :comment_id {:type "string"}} :required ["file_key" "comment_id"]}}
   {:name "post_comment_reaction" :description "WRITE MODE ONLY. Add a reaction to a comment." :inputSchema {:type "object" :properties {:file_key {:type "string"} :comment_id {:type "string"} :emoji {:type "string"}} :required ["file_key" "comment_id" "emoji"]}}
   {:name "delete_comment_reaction" :description "WRITE MODE ONLY. Delete a reaction from a comment." :inputSchema {:type "object" :properties {:file_key {:type "string"} :comment_id {:type "string"} :emoji {:type "string"}} :required ["file_key" "comment_id" "emoji"]}}
   {:name "post_variables" :description "WRITE MODE ONLY. Bulk create, update, and delete variables." :inputSchema {:type "object" :properties {:file_key {:type "string"} :changes {:type "object"}} :required ["file_key" "changes"]}}
   {:name "post_dev_resources" :description "WRITE MODE ONLY. Bulk create dev resources." :inputSchema {:type "object" :properties {:dev_resources {:type "array" :items {:type "object"}}} :required ["dev_resources"]}}
   {:name "put_dev_resources" :description "WRITE MODE ONLY. Bulk update dev resources." :inputSchema {:type "object" :properties {:dev_resources {:type "array" :items {:type "object"}}} :required ["dev_resources"]}}
   {:name "create_dev_resource" :description "WRITE MODE ONLY. Create a single dev resource link through the official bulk API." :inputSchema {:type "object" :properties {:file_key {:type "string"} :name {:type "string"} :url {:type "string"} :node_id {:type "string"}} :required ["file_key" "name" "url" "node_id"]}}
   {:name "delete_dev_resource" :description "WRITE MODE ONLY. Delete a dev resource link." :inputSchema {:type "object" :properties {:file_key {:type "string"} :dev_resource_id {:type "string"}} :required ["file_key" "dev_resource_id"]}}
   {:name "create_webhook" :description "WRITE MODE ONLY. Create a webhook." :inputSchema {:type "object" :properties {:event_type {:type "string"} :team_id {:type "string"} :endpoint {:type "string"} :passcode {:type "string"} :description {:type "string"} :status {:type "string"}} :required ["event_type" "team_id" "endpoint"]}}
   {:name "update_webhook" :description "WRITE MODE ONLY. Update a webhook." :inputSchema {:type "object" :properties {:webhook_id {:type "string"} :webhook {:type "object"}} :required ["webhook_id" "webhook"]}}
   {:name "delete_webhook" :description "WRITE MODE ONLY. Delete a webhook." :inputSchema {:type "object" :properties {:webhook_id {:type "string"}} :required ["webhook_id"]}}])

;; --- Tool handlers ---

(defn- ->json [data]
  (js/JSON.stringify (clj->js data) nil 2))

(defn- success [text]
  #js {:content #js [#js {:type "text" :text text}]})

(defn- error [msg]
  #js {:content #js [#js {:type "text" :text (str "Error: " msg)}]
       :isError true})

(def ^:private write-tool-names
  (delay (set (map :name write-tools))))

(def ^:private missing-token-message
  "fighorse needs a Figma Personal Access Token before calling Figma APIs. Run `fighorse auth login --token <FIGMA_TOKEN>` or set FIGMA_TOKEN, then call check_fighorse_ready again.")

(defn- get-token []
  (let [token (:token (config/load-config))]
    (when (str/blank? token)
      (throw (js/Error. missing-token-message)))
    token))

(defn- handle-promise
  "Execute a promise and return MCP result."
  [p]
  (-> p
      (.then (fn [data] (success (->json data))))
      (.catch (fn [err] (error (or (.-message err) (str err)))))))

(defmulti handle-tool (fn [name _args] name))

;; --- Self discovery / AI replication ---
(defmethod handle-tool "discover_fighorse" [_ _]
  (js/Promise.resolve (success (->json (discovery/manifest)))))

(defmethod handle-tool "check_fighorse_ready" [_ _]
  (js/Promise.resolve (success (->json (discovery/doctor)))))

(defmethod handle-tool "parse_figma_url" [_ args]
  (js/Promise.resolve (success (->json (figma-url/parse-figma-url (:figma_url args))))))

(defmethod handle-tool "get_replicate_workflow" [_ _]
  (js/Promise.resolve (success (->json (discovery/workflow)))))

(defmethod handle-tool "get_experience_schema" [_ _]
  (js/Promise.resolve (success (->json (experience/schema)))))

(defmethod handle-tool "list_experiences" [_ args]
  (js/Promise.resolve
   (success (->json (experience/guidance :platform (:platform args)
                                          :asset-format (:asset_format args)
                                          :category (:category args)
                                          :tag (:tag args)
                                          :scope (:scope args)
                                          :project-dir (:project_dir args)
                                          :limit (or (:limit args) 6))))))

(defmethod handle-tool "record_experience" [_ args]
  (try
    (js/Promise.resolve (success (->json (experience/add! args
                                                          {:scope (:scope args)
                                                           :project-dir (:project_dir args)}))))
    (catch :default err
      (js/Promise.resolve (error (or (.-message err) (str err)))))))

(defmethod handle-tool "get_design_package" [_ args]
  (handle-promise
   (design-package/get-design-package (get-token)
                                      :figma-url (:figma_url args)
                                      :file-key (:file_key args)
                                      :node-id (:node_id args)
                                      :depth (:depth args)
                                      :max-tokens (:max_tokens args)
                                      :include-screenshot (:include_screenshot args)
                                      :include-assets (:include_assets args)
                                      :screenshot-format (or (:screenshot_format args) "png")
                                      :scale (:scale args)
                                      :platform (:platform args)
                                      :asset-format (:asset_format args))))

(defmethod handle-tool "visual_audit" [_ args]
  (js/Promise.resolve
   (success (->json (visual-audit/audit :figma-url (:figma_url args)
                                        :screenshot-path (:screenshot_path args)
                                        :platform (:platform args)
                                        :asset-format (:asset_format args)
                                        :notes (:notes args))))))

(defmethod handle-tool "get_project_playbook" [_ args]
  (js/Promise.resolve
   (success (->json (playbook/build :platform (:platform args)
                                    :asset-format (:asset_format args)
                                    :project-dir (:project_dir args))))))

;; --- Files ---
(defmethod handle-tool "get_file" [_ args]
  (handle-promise
   (files-api/get-file (get-token) (:file_key args)
                       :depth (:depth args)
                       :version (:version args))))

(defmethod handle-tool "get_file_nodes" [_ args]
  (handle-promise
   (files-api/get-file-nodes (get-token) (:file_key args) (:node_ids args)
                             :depth (:depth args))))

(defmethod handle-tool "get_file_compact" [_ args]
  (let [token (get-token)
        file-key (:file_key args)
        max-tokens (:max_tokens args)
        depth (:depth args)]
    (-> (files-api/get-file token file-key :depth depth)
        (.then (fn [data]
                 (let [node (figma/response->node data)
                       compacted (compact/compact node
                                                   :extractors compact/default-extractors
                                                   :max-depth depth
                                                   :max-tokens max-tokens)]
                   (success (->json compacted)))))
        (.catch (fn [err] (error (or (.-message err) (str err))))))))

(defmethod handle-tool "get_design_context" [_ args]
  (handle-tool "get_file_compact" args))

(defmethod handle-tool "get_node" [_ args]
  (handle-promise
   (files-api/get-file-nodes (get-token) (:file_key args) (:node_id args)
                             :depth (:depth args))))

(defmethod handle-tool "get_tokens" [_ args]
  (let [token (get-token)
        file-key (:file_key args)
        depth (:depth args)]
    (-> (files-api/get-file token file-key :depth (or depth 2))
        (.then (fn [data]
                 (let [doc (get-in data [:document])
                       simplified (compact/simplify-tree doc compact/default-extractors depth)
                       extracted (tokens/extract-tokens simplified)]
                   (success (->json (tokens/tokens-by-category extracted))))))
        (.catch (fn [err] (error (or (.-message err) (str err))))))))

(defmethod handle-tool "get_file_tree" [_ args]
  (let [token (get-token)
        file-key (:file_key args)
        depth (:depth args)]
    (-> (files-api/get-file token file-key :depth (or depth 2))
        (.then (fn [data]
                 (let [doc (get-in data [:document])
                       tree (compact/simplify-tree doc
                                                    [compact/dimension-extractor
                                                     compact/layout-extractor]
                                                    depth)]
                   (success (->json tree)))))
        (.catch (fn [err] (error (or (.-message err) (str err))))))))

(defmethod handle-tool "get_file_meta" [_ args]
  (handle-promise (files-api/get-file-meta (get-token) (:file_key args))))

(defmethod handle-tool "get_file_versions" [_ args]
  (handle-promise
   (files-api/get-file-versions (get-token) (:file_key args)
                                :page-size (:page_size args))))

(defmethod handle-tool "get_images" [_ args]
  (handle-promise
   (files-api/get-images (get-token) (:file_key args) (:node_ids args)
                         :format (or (:format args) "png")
                         :scale (or (:scale args) 2))))

(defmethod handle-tool "get_screenshot" [_ args]
  (handle-tool "get_images" args))

(defmethod handle-tool "export_images" [_ args]
  (handle-promise
   (img-export/export-images (get-token)
                             (:file_key args)
                             (str/split (:node_ids args) #",")
                             :format (or (:format args) "png")
                             :scale (or (:scale args) 2)
                             :dest-dir (:dest_dir args)
                             :manifest (true? (:manifest args))
                             :prefix (:prefix args))))

(defmethod handle-tool "export_component" [_ args]
  (handle-tool "export_images" args))

(defmethod handle-tool "get_image_fills" [_ args]
  (handle-promise (files-api/get-image-fills (get-token) (:file_key args))))

(defmethod handle-tool "download_image_fills" [_ args]
  (handle-promise
   (img-export/download-image-fills (get-token)
                                    (:file_key args)
                                    :dest-dir (:dest_dir args)
                                    :manifest (true? (:manifest args))
                                    :prefix (:prefix args))))

;; --- Projects ---
(defmethod handle-tool "get_team_projects" [_ args]
  (handle-promise (projects-api/get-team-projects (get-token) (:team_id args))))

(defmethod handle-tool "get_project_files" [_ args]
  (handle-promise (projects-api/get-project-files (get-token) (:project_id args))))

;; --- Users ---
(defmethod handle-tool "get_me" [_ _]
  (handle-promise (users-api/get-me (get-token))))

;; --- Components ---
(defmethod handle-tool "get_team_components" [_ args]
  (handle-promise
   (components-api/get-team-components (get-token) (:team_id args)
                                       :page-size (:page_size args))))

(defmethod handle-tool "get_file_components" [_ args]
  (handle-promise (components-api/get-file-components (get-token) (:file_key args))))

(defmethod handle-tool "get_component" [_ args]
  (handle-promise (components-api/get-component (get-token) (:component_key args))))

(defmethod handle-tool "get_file_component_sets" [_ args]
  (handle-promise (components-api/get-file-component-sets (get-token) (:file_key args))))

(defmethod handle-tool "get_team_component_sets" [_ args]
  (handle-promise
   (components-api/get-team-component-sets (get-token) (:team_id args)
                                          :page-size (:page_size args))))

(defmethod handle-tool "get_component_set" [_ args]
  (handle-promise (components-api/get-component-set (get-token) (:component_set_key args))))

;; --- Styles ---
(defmethod handle-tool "get_team_styles" [_ args]
  (handle-promise
   (styles-api/get-team-styles (get-token) (:team_id args)
                               :page-size (:page_size args))))

(defmethod handle-tool "get_file_styles" [_ args]
  (handle-promise (styles-api/get-file-styles (get-token) (:file_key args))))

(defmethod handle-tool "get_style" [_ args]
  (handle-promise (styles-api/get-style (get-token) (:style_key args))))

;; --- Comments ---
(defmethod handle-tool "get_comments" [_ args]
  (handle-promise
   (comments-api/get-comments (get-token) (:file_key args)
                              :as-md (:as_md args))))

(defmethod handle-tool "post_comment" [_ args]
  (handle-promise
   (comments-api/post-comment (get-token) (:file_key args) (:message args)
                              :comment-id (:reply_to args))))

(defmethod handle-tool "delete_comment" [_ args]
  (handle-promise
   (comments-api/delete-comment (get-token) (:file_key args) (:comment_id args))))

(defmethod handle-tool "get_comment_reactions" [_ args]
  (handle-promise
   (comments-api/get-comment-reactions (get-token) (:file_key args) (:comment_id args)
                                       :cursor (:cursor args))))

(defmethod handle-tool "post_comment_reaction" [_ args]
  (handle-promise
   (comments-api/post-comment-reaction (get-token) (:file_key args) (:comment_id args) (:emoji args))))

(defmethod handle-tool "delete_comment_reaction" [_ args]
  (handle-promise
   (comments-api/delete-comment-reaction (get-token) (:file_key args) (:comment_id args) (:emoji args))))

;; --- Variables ---
(defmethod handle-tool "get_local_variables" [_ args]
  (handle-promise (variables-api/get-local-variables (get-token) (:file_key args))))

(defmethod handle-tool "get_published_variables" [_ args]
  (handle-promise (variables-api/get-published-variables (get-token) (:file_key args))))

(defmethod handle-tool "post_variables" [_ args]
  (handle-promise
   (variables-api/post-variables (get-token) (:file_key args) (:changes args))))

;; --- Dev Resources ---
(defmethod handle-tool "get_dev_resources" [_ args]
  (handle-promise (dev-resources-api/get-dev-resources (get-token) (:file_key args)
                                                       :node-ids (:node_ids args))))

(defmethod handle-tool "post_dev_resources" [_ args]
  (handle-promise
   (dev-resources-api/post-dev-resources (get-token) (:dev_resources args))))

(defmethod handle-tool "put_dev_resources" [_ args]
  (handle-promise
   (dev-resources-api/put-dev-resources (get-token) (:dev_resources args))))

(defmethod handle-tool "create_dev_resource" [_ args]
  (handle-promise
   (dev-resources-api/create-dev-resource (get-token) (:file_key args) (:name args) (:url args) (:node_id args))))

(defmethod handle-tool "delete_dev_resource" [_ args]
  (handle-promise
   (dev-resources-api/delete-dev-resource (get-token) (:file_key args) (:dev_resource_id args))))

;; --- Webhooks ---
(defmethod handle-tool "get_webhooks" [_ args]
  (handle-promise (webhooks-api/get-webhooks (get-token) :context (:context args))))

(defmethod handle-tool "get_webhook" [_ args]
  (handle-promise (webhooks-api/get-webhook (get-token) (:webhook_id args))))

(defmethod handle-tool "get_team_webhooks" [_ args]
  (handle-promise (webhooks-api/get-team-webhooks (get-token) (:team_id args))))

(defmethod handle-tool "get_webhook_requests" [_ args]
  (handle-promise (webhooks-api/get-webhook-requests (get-token) (:webhook_id args)
                                                     :cursor (:cursor args))))

(defmethod handle-tool "create_webhook" [_ args]
  (handle-promise
   (webhooks-api/create-webhook (get-token) (:event_type args) (:team_id args) (:endpoint args)
                                :passcode (:passcode args)
                                :description (:description args)
                                :status (:status args))))

(defmethod handle-tool "update_webhook" [_ args]
  (handle-promise (webhooks-api/update-webhook (get-token) (:webhook_id args) (:webhook args))))

(defmethod handle-tool "delete_webhook" [_ args]
  (handle-promise (webhooks-api/delete-webhook (get-token) (:webhook_id args))))

;; --- Admin / Analytics / Payments / oEmbed ---
(defmethod handle-tool "get_activity_logs" [_ args]
  (handle-promise
   (activity-logs-api/get-activity-logs (get-token)
                                        :start-time (:start_time args)
                                        :end-time (:end_time args)
                                        :events (:events args)
                                        :limit (:limit args)
                                        :order (:order args))))

(defmethod handle-tool "get_developer_logs" [_ args]
  (handle-promise
   (developer-logs-api/get-developer-logs (get-token)
                                          :token-type (:token_type args)
                                          :token-value (:token args)
                                          :token-name (:token_name args)
                                          :user-email (:user_email args)
                                          :ip-address (:ip_address args)
                                          :event-source (:event_source args)
                                          :date-range (:date_range args)
                                          :limit (:limit args)
                                          :cursor (:cursor args))))

(defmethod handle-tool "get_payments" [_ args]
  (handle-promise
   (payments-api/get-payments (get-token)
                              :plugin-payment-token (:plugin_payment_token args)
                              :user-id (:user_id args)
                              :community-file-id (:community_file_id args)
                              :plugin-id (:plugin_id args)
                              :widget-id (:widget_id args))))

(defmethod handle-tool "get_oembed" [_ args]
  (handle-promise
   (oembed-api/get-oembed :url (:url args)
                          :max-width (:max_width args)
                          :max-height (:max_height args))))

(defmethod handle-tool "get_library_analytics_component_usages" [_ args]
  (handle-promise (analytics-api/get-library-analytics-component-usages (get-token) (:file_key args))))

(defmethod handle-tool "get_library_analytics_component_actions" [_ args]
  (handle-promise (analytics-api/get-library-analytics-component-actions (get-token) (:file_key args))))

(defmethod handle-tool "get_library_analytics_style_usages" [_ args]
  (handle-promise (analytics-api/get-library-analytics-style-usages (get-token) (:file_key args))))

(defmethod handle-tool "get_library_analytics_style_actions" [_ args]
  (handle-promise
   (analytics-api/get-library-analytics-style-actions (get-token) (:file_key args)
                                                     :group-by (:group_by args))))

(defmethod handle-tool "get_library_analytics_variable_usages" [_ args]
  (handle-promise (analytics-api/get-library-analytics-variable-usages (get-token) (:file_key args))))

(defmethod handle-tool "get_library_analytics_variable_actions" [_ args]
  (handle-promise (analytics-api/get-library-analytics-variable-actions (get-token) (:file_key args))))

;; --- Fallback ---
(defmethod handle-tool :default [name _]
  (js/Promise.resolve (error (str "Unknown tool: " name))))

;; --- Public API ---

(defn list-tools
  "Return all registered tool definitions for tools/list."
  []
  (let [official-tools (if (config/mcp-write-enabled?)
                         (registry/official-tools)
                         (remove #(registry/write-tool-name? (:name %))
                                 (registry/official-tools)))
        tools (concat all-tools extra-tools
                      official-tools
                      (when (config/mcp-write-enabled?)
                        write-tools))]
    (clj->js {:tools (mapv #(select-keys % [:name :description :inputSchema])
                           tools)})))

(defn call-tool
  "Execute a tool call for tools/call."
  [^js request]
  (try
    (let [name (.-name (.-params request))
          args (js->clj (.-arguments (.-params request)) :keywordize-keys true)]
      (if-let [msg (policy/violation @write-tool-names name)]
        (js/Promise.resolve (error msg))
        (if (registry/official-tool-name? name)
          (let [operation-id (registry/operation-id-for-tool name)
                params (or (:params args) {})
                body (:body args)]
            (handle-promise
             (-> (operations/call-operation (get-token) operation-id params body)
                 (.then (fn [data]
                          (if (:ai_guidance args)
                            (operations/result-envelope operation-id data)
                            data))))))
          (handle-tool name args))))
    (catch :default err
      (js/Promise.resolve (error (or (.-message err) (str err)))))))

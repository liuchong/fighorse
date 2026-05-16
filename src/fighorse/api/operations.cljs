(ns fighorse.api.operations
  "Operation-id dispatcher for the public Figma REST API."
  (:require [clojure.string :as str]
            [fighorse.api.activity-logs :as activity-logs-api]
            [fighorse.api.analytics :as analytics-api]
            [fighorse.api.comments :as comments-api]
            [fighorse.api.components :as components-api]
            [fighorse.api.coverage :as coverage]
            [fighorse.api.dev-resources :as dev-resources-api]
            [fighorse.api.developer-logs :as developer-logs-api]
            [fighorse.api.files :as files-api]
            [fighorse.api.oembed :as oembed-api]
            [fighorse.api.payments :as payments-api]
            [fighorse.api.projects :as projects-api]
            [fighorse.api.styles :as styles-api]
            [fighorse.api.users :as users-api]
            [fighorse.api.variables :as variables-api]
            [fighorse.api.webhooks :as webhooks-api]))

(defn operation [operation-id]
  (get coverage/operations-by-id operation-id))

(defn write-operation? [operation-id]
  (some-> (operation operation-id) coverage/write-operation?))

(defn- snake->kebab [value]
  (keyword (str/replace (name value) "_" "-")))

(defn- param
  ([params k] (param params k nil))
  ([params k default]
   (let [string-key (name k)]
     (or (get params k)
         (get params (snake->kebab k))
         (get params string-key)
         (get params (str/replace string-key "_" "-"))
         default))))

(defn- params-or-empty [params]
  (or params {}))

(defn- body-or-empty [body]
  (or body {}))

(defn- dev-resources-body [body]
  (or (:dev_resources body)
      (:dev-resources body)
      (get body "dev_resources")
      (get body "dev-resources")
      []))

(defn call-operation
  "Call an official Figma REST operation by operationId.
   params is for path/query values; body is the JSON request body for write APIs."
  ([token operation-id params]
   (call-operation token operation-id params nil))
  ([token operation-id params body]
   (let [params (params-or-empty params)
         body (body-or-empty body)]
     (case operation-id
       "getFile"
       (files-api/get-file token (param params :file_key)
                           :version (param params :version)
                           :ids (param params :ids)
                           :depth (param params :depth)
                           :geometry (param params :geometry)
                           :plugin-data (param params :plugin_data)
                           :branch-data (param params :branch_data))

       "getFileNodes"
       (files-api/get-file-nodes token (param params :file_key) (param params :ids)
                                 :version (param params :version)
                                 :depth (param params :depth)
                                 :geometry (param params :geometry)
                                 :plugin-data (param params :plugin_data))

       "getImages"
       (files-api/get-images token (param params :file_key) (param params :ids)
                             :version (param params :version)
                             :scale (param params :scale)
                             :format (param params :format)
                             :svg-outline-text (param params :svg_outline_text)
                             :svg-include-id (param params :svg_include_id)
                             :svg-include-node-id (param params :svg_include_node_id)
                             :svg-simplify-stroke (param params :svg_simplify_stroke)
                             :contents-only (param params :contents_only)
                             :use-absolute-bounds (param params :use_absolute_bounds))

       "getImageFills"
       (files-api/get-image-fills token (param params :file_key))

       "getFileMeta"
       (files-api/get-file-meta token (param params :file_key))

       "getTeamProjects"
       (projects-api/get-team-projects token (param params :team_id))

       "getProjectFiles"
       (projects-api/get-project-files token (param params :project_id)
                                       :branch-data (param params :branch_data))

       "getFileVersions"
       (files-api/get-file-versions token (param params :file_key)
                                    :page-size (param params :page_size)
                                    :before (param params :before)
                                    :after (param params :after))

       "getComments"
       (comments-api/get-comments token (param params :file_key)
                                  :as-md (param params :as_md))

       "postComment"
       (comments-api/post-comment token (param params :file_key)
                                  (or (:message body) (param params :message))
                                  :comment-id (or (:comment_id body) (param params :comment_id))
                                  :client-meta (or (:client_meta body) (:client-meta body)))

       "deleteComment"
       (comments-api/delete-comment token (param params :file_key) (param params :comment_id))

       "getCommentReactions"
       (comments-api/get-comment-reactions token (param params :file_key) (param params :comment_id)
                                           :cursor (param params :cursor))

       "postCommentReaction"
       (comments-api/post-comment-reaction token (param params :file_key) (param params :comment_id)
                                           (or (:emoji body) (param params :emoji)))

       "deleteCommentReaction"
       (comments-api/delete-comment-reaction token (param params :file_key) (param params :comment_id)
                                             (or (:emoji body) (param params :emoji)))

       "getMe"
       (users-api/get-me token)

       "getTeamComponents"
       (components-api/get-team-components token (param params :team_id)
                                           :page-size (param params :page_size)
                                           :after (param params :after)
                                           :before (param params :before))

       "getFileComponents"
       (components-api/get-file-components token (param params :file_key))

       "getComponent"
       (components-api/get-component token (param params :key))

       "getTeamComponentSets"
       (components-api/get-team-component-sets token (param params :team_id)
                                               :page-size (param params :page_size)
                                               :after (param params :after)
                                               :before (param params :before))

       "getFileComponentSets"
       (components-api/get-file-component-sets token (param params :file_key))

       "getComponentSet"
       (components-api/get-component-set token (param params :key))

       "getTeamStyles"
       (styles-api/get-team-styles token (param params :team_id)
                                   :page-size (param params :page_size)
                                   :after (param params :after)
                                   :before (param params :before))

       "getFileStyles"
       (styles-api/get-file-styles token (param params :file_key))

       "getStyle"
       (styles-api/get-style token (param params :key))

       "getWebhooks"
       (webhooks-api/get-webhooks token
                                  :context (param params :context)
                                  :plan (param params :plan)
                                  :after (param params :after))

       "postWebhook"
       (webhooks-api/create-webhook token body)

       "getWebhook"
       (webhooks-api/get-webhook token (param params :webhook_id))

       "putWebhook"
       (webhooks-api/update-webhook token (param params :webhook_id) body)

       "deleteWebhook"
       (webhooks-api/delete-webhook token (param params :webhook_id))

       "getTeamWebhooks"
       (webhooks-api/get-team-webhooks token (param params :team_id))

       "getWebhookRequests"
       (webhooks-api/get-webhook-requests token (param params :webhook_id)
                                          :cursor (param params :cursor))

       "getActivityLogs"
       (activity-logs-api/get-activity-logs token
                                            :start-time (param params :start_time)
                                            :end-time (param params :end_time)
                                            :events (param params :events)
                                            :limit (param params :limit)
                                            :order (param params :order))

       "getDeveloperLogs"
       (developer-logs-api/get-developer-logs token
                                              :token-type (or (:token_type body) (param params :token_type))
                                              :token-value (or (:token body) (param params :token))
                                              :token-name (or (:token_name body) (param params :token_name))
                                              :user-email (or (:user_email body) (param params :user_email))
                                              :ip-address (or (:ip_address body) (param params :ip_address))
                                              :event-source (or (:event_source body) (param params :event_source))
                                              :date-range (or (:date_range body) (param params :date_range))
                                              :limit (or (:limit body) (param params :limit))
                                              :cursor (or (:cursor body) (param params :cursor)))

       "getPayments"
       (payments-api/get-payments token
                                  :plugin-payment-token (param params :plugin_payment_token)
                                  :user-id (param params :user_id)
                                  :community-file-id (param params :community_file_id)
                                  :plugin-id (param params :plugin_id)
                                  :widget-id (param params :widget_id))

       "getLocalVariables"
       (variables-api/get-local-variables token (param params :file_key))

       "getPublishedVariables"
       (variables-api/get-published-variables token (param params :file_key))

       "postVariables"
       (variables-api/post-variables token (param params :file_key) body)

       "getDevResources"
       (dev-resources-api/get-dev-resources token (param params :file_key)
                                            :node-ids (param params :node_ids))

       "postDevResources"
       (dev-resources-api/post-dev-resources token (dev-resources-body body))

       "putDevResources"
       (dev-resources-api/put-dev-resources token (dev-resources-body body))

       "deleteDevResource"
       (dev-resources-api/delete-dev-resource token (param params :file_key) (param params :dev_resource_id))

       "getLibraryAnalyticsComponentActions"
       (analytics-api/get-library-analytics-component-actions token (param params :file_key)
                                                              :cursor (param params :cursor)
                                                              :start-date (param params :start_date)
                                                              :end-date (param params :end_date)
                                                              :order-direction (param params :order_direction))

       "getLibraryAnalyticsComponentUsages"
       (analytics-api/get-library-analytics-component-usages token (param params :file_key)
                                                             :cursor (param params :cursor)
                                                             :start-date (param params :start_date)
                                                             :end-date (param params :end_date)
                                                             :order-direction (param params :order_direction))

       "getLibraryAnalyticsStyleActions"
       (analytics-api/get-library-analytics-style-actions token (param params :file_key)
                                                          :cursor (param params :cursor)
                                                          :group-by (param params :group_by)
                                                          :start-date (param params :start_date)
                                                          :end-date (param params :end_date))

       "getLibraryAnalyticsStyleUsages"
       (analytics-api/get-library-analytics-style-usages token (param params :file_key)
                                                         :cursor (param params :cursor)
                                                         :start-date (param params :start_date)
                                                         :end-date (param params :end_date)
                                                         :order-direction (param params :order_direction))

       "getLibraryAnalyticsVariableActions"
       (analytics-api/get-library-analytics-variable-actions token (param params :file_key)
                                                             :cursor (param params :cursor)
                                                             :start-date (param params :start_date)
                                                             :end-date (param params :end_date)
                                                             :order-direction (param params :order_direction))

       "getLibraryAnalyticsVariableUsages"
       (analytics-api/get-library-analytics-variable-usages token (param params :file_key)
                                                            :cursor (param params :cursor)
                                                            :start-date (param params :start_date)
                                                            :end-date (param params :end_date)
                                                            :order-direction (param params :order_direction))

       "getOembed"
       (oembed-api/get-oembed :url (param params :url)
                              :max-width (param params :max_width)
                              :max-height (param params :max_height))

       (js/Promise.reject (js/Error. (str "Unknown Figma operationId: " operation-id)))))))

(defn result-envelope [operation-id data & {:keys [ai-guidance]}]
  (let [op (operation operation-id)]
    {:kind "fighorse.api-result.v1"
     :operation (select-keys op [:operation_id :method :path :tag :deprecated])
     :data data
     :ai_guidance (or ai-guidance
                      {:summary (str "Figma " (:method op) " " (:path op) " completed.")
                       :next_step "Use the data directly, or call discover_fighorse/get_design_package when you need AI-optimized implementation context."})}))

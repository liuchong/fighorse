(ns fighorse.core
  "CLI entry point. Routes commands to API modules + AI enhancements."
  (:require [clojure.string :as str]
            [fighorse.config :as config]
            [fighorse.compact :as compact]
            [fighorse.design-package :as design-package]
            [fighorse.discovery :as discovery]
            [fighorse.diff :as diff]
            [fighorse.experience :as experience]
            [fighorse.filter :as tree-filter]
            [fighorse.figma :as figma]
            [fighorse.install :as install]
            [fighorse.schema :as schema]
            [fighorse.api.coverage :as api-coverage]
            [fighorse.api.operations :as api-operations]
            [fighorse.api.files :as files-api]
            [fighorse.api.projects :as projects-api]
            [fighorse.api.users :as users-api]
            [fighorse.api.components :as components-api]
            [fighorse.api.styles :as styles-api]
            [fighorse.api.comments :as comments-api]
            [fighorse.api.webhooks :as webhooks-api]
            [fighorse.api.variables :as variables-api]
            [fighorse.api.dev-resources :as dev-resources-api]
            [fighorse.export.md :as md-export]
            [fighorse.export.images :as img-export]
            [fighorse.tokens :as tokens]
            [fighorse.utils.url :as figma-url]
            [fighorse.utils.http :as http-client]
            [fighorse.mcp.server :as mcp]
            [fighorse.product.playbook :as playbook]
            [fighorse.product.visual-audit :as visual-audit]))

(def ^:private fs (js/require "fs"))
(def ^:private path (js/require "path"))
(defonce ^:private cli-exiting (atom false))
(defonce ^:private cli-signal-handlers-installed (atom false))

;; --- Output ---

(defn- json-str [data]
  (js/JSON.stringify (clj->js data) nil 2))

(defn- eprintln [& parts]
  (.error js/console (str/join " " (map str parts))))

(defn- explicit-cli-exit-enabled? []
  (not= "0" (.-FIGHORSE_CLI_EXPLICIT_EXIT js/process.env)))

(defn- finish-cli!
  ([] (finish-cli! 0))
  ([code]
   (when (and (explicit-cli-exit-enabled?)
              (compare-and-set! cli-exiting false true))
     (js/setTimeout (fn [] (js/process.exit code)) 25))))

(defn- fail-cli! [& parts]
  (apply eprintln parts)
  (http-client/abort-active-requests!)
  (finish-cli! 1))

(defn- install-cli-signal-handlers! []
  (when (and (explicit-cli-exit-enabled?)
             (compare-and-set! cli-signal-handlers-installed false true))
    (doseq [signal ["SIGINT" "SIGTERM"]]
      (.once js/process signal
             (fn []
               (http-client/abort-active-requests!)
               (finish-cli! 130))))))

;; --- Arg parsing ---

(defn- parse-flag
  "Find flag value in args. Returns [value clean-args]."
  [args flag]
  (let [idx (.indexOf args flag)]
    (if (>= idx 0)
      [(nth args (inc idx) nil)
       (into [] (comp (map-indexed vector)
                      (filter #(not (#{idx (inc idx)} (first %))))
                      (map second))
             args)]
      [nil args])))

(defn- parse-flags [args flags]
  (loop [remaining args
         result {}
         flags flags]
    (if (empty? flags)
      [result remaining]
      (let [flag (first flags)
            [val clean] (parse-flag remaining flag)]
        (recur clean (assoc result (keyword (str/replace (subs flag 2) #"-" "_")) val) (rest flags))))))

(defn- flag-present? [args flag]
  (some #(= flag %) args))

;; --- Command helpers ---

(defn- require-token! []
  (let [{:keys [token]} (config/load-config)]
    (when (str/blank? token)
      (eprintln "Error: FIGMA_TOKEN or FIGMA_API_KEY environment variable required")
      (js/process.exit 1))
    token))

(defn- require-arg [args idx name]
  (let [v (nth args idx nil)]
    (when (str/blank? v)
      (eprintln (str "Error: " name " required"))
      (js/process.exit 1))
    v))

(defn- require-value [value name]
  (when (str/blank? value)
    (eprintln (str "Error: " name " required"))
    (js/process.exit 1))
  value)

(defn- optional-int [s]
  (when s
    (let [n (js/parseInt s)]
      (when-not (js/isNaN n)
        n))))

(defn- optional-float [s]
  (when s
    (let [n (js/parseFloat s)]
      (when-not (js/isNaN n)
        n))))

(defn- compact-map [m]
  (not-empty
   (into {}
         (keep (fn [[k v]]
                 (cond
                   (nil? v) nil
                   (and (string? v) (str/blank? v)) nil
                   (and (coll? v) (empty? v)) nil
                   :else [k v])))
         m)))

(defn- parse-json-map! [raw label]
  (when-not (str/blank? raw)
    (try
      (let [data (js->clj (js/JSON.parse raw) :keywordize-keys true)]
        (if (map? data)
          data
          (throw (js/Error. (str label " must be a JSON object")))))
      (catch :default err
        (eprintln "Error:" (or (.-message err) (str err)))
        (js/process.exit 1)))))

(declare print-data)

(defn- print-json [p & {:keys [output]}]
  (-> p
      (.then (fn [data] (print-data data :output output)))
      (.catch (fn [err]
                (fail-cli! "Error:" (or (.-message err) (str err)))))))

(defn- write-output! [content output]
  (if output
    (do
      (.mkdirSync fs (.dirname path output) #js {:recursive true})
      (.writeFileSync fs output content))
    (println content))
  (finish-cli!))

(defn- print-data [data & {:keys [output]}]
  (write-output! (json-str data) output))

(defn- read-stdin []
  (.readFileSync fs 0 "utf8"))

;; --- Experience / learning commands ---

(defn cmd-experience-schema [args]
  (let [[flags _] (parse-flags args ["--output"])]
    (print-data (experience/schema) :output (:output flags))))

(defn cmd-experience-path [args]
  (let [[flags _] (parse-flags args ["--scope" "--project-dir" "--output"])]
    (print-data {:kind "fighorse.experience-store.v1"
                 :path (experience/experience-path :scope (:scope flags)
                                                   :project-dir (:project_dir flags))
                 :store (experience/store-info :scope (:scope flags)
                                                :project-dir (:project_dir flags))
                 :schema_version experience/schema-version
                 :records (count (experience/read-all {:scope (:scope flags)
                                                        :project-dir (:project_dir flags)}))}
                :output (:output flags))))

(defn cmd-experience-add [args]
  (let [[flags _] (parse-flags args ["--summary" "--lesson" "--category" "--severity"
                                      "--platform" "--asset-format" "--figma-url"
                                      "--file-key" "--node-id" "--tags" "--evidence"
                                      "--recommendation" "--client" "--command"
                                      "--json" "--scope" "--project-dir" "--output"])
        stdin-json (when (and (not (:json flags))
                              (not (:summary flags))
                              (not (.-isTTY js/process.stdin)))
                     (read-stdin))
        base (or (parse-json-map! (or (:json flags) stdin-json) "experience") {})
        record (merge base
                      (compact-map {:summary (:summary flags)
                                    :lesson (:lesson flags)
                                    :category (:category flags)
                                    :severity (:severity flags)
                                    :platform (:platform flags)
                                    :asset_format (:asset_format flags)
                                    :figma_url (:figma_url flags)
                                    :file_key (:file_key flags)
                                    :node_id (:node_id flags)
                                    :tags (:tags flags)
                                    :evidence (:evidence flags)
                                    :recommendation (:recommendation flags)
                                    :client (:client flags)
                                    :command (:command flags)}))]
    (try
      (print-data (experience/add! record {:scope (:scope flags)
                                           :project-dir (:project_dir flags)})
                  :output (:output flags))
      (catch :default err
        (println "Error:" (or (.-message err) (str err)))
        (js/process.exit 1)))))

(defn cmd-experience-list [args]
  (let [[flags _] (parse-flags args ["--platform" "--asset-format" "--category"
                                      "--tag" "--limit" "--scope" "--project-dir" "--output"])
        limit (optional-int (:limit flags))]
    (print-data (experience/list-experiences :platform (:platform flags)
                                             :asset-format (:asset_format flags)
                                             :category (:category flags)
                                             :tag (:tag flags)
                                             :scope (:scope flags)
                                             :project-dir (:project_dir flags)
                                             :limit (or limit 8))
                :output (:output flags))))

(defn cmd-experience-summary [args]
  (let [[flags _] (parse-flags args ["--platform" "--asset-format" "--category"
                                      "--tag" "--limit" "--format" "--scope" "--project-dir" "--output"])
        limit (optional-int (:limit flags))
        data (experience/guidance :platform (:platform flags)
                                  :asset-format (:asset_format flags)
                                  :category (:category flags)
                                  :tag (:tag flags)
                                  :scope (:scope flags)
                                  :project-dir (:project_dir flags)
                                  :limit (or limit 6))]
    (if (= "md" (:format flags))
      (write-output! (experience/guidance->markdown data) (:output flags))
      (print-data data :output (:output flags)))))

;; --- Installation commands ---

(defn cmd-install-self [args]
  (let [[flags _] (parse-flags args ["--source" "--path" "--target" "--client" "--clients"
                                     "--transport" "--port" "--command" "--home" "--token"
                                     "--mode" "--service" "--link-dir" "--link-dirs" "--output"])
        apply (boolean (flag-present? args "--apply"))
        default? (boolean (flag-present? args "--default"))
        port (optional-int (:port flags))]
    (print-data (install/install-self! :source (:source flags)
                                       :path (:path flags)
                                       :target (:target flags)
                                       :default? default?
                                       :client (:client flags)
                                       :clients (:clients flags)
                                       :transport (or (:transport flags) "http")
                                       :port (or port 9449)
                                       :command (or (:command flags) "fighorse")
                                       :home (:home flags)
                                       :token (:token flags)
                                       :mode (:mode flags)
                                       :service (or (:service flags) "auto")
                                       :link-dir (:link_dir flags)
                                       :link-dirs (:link_dirs flags)
                                       :apply apply)
                :output (:output flags))))

(defn cmd-install-home [args]
  (let [[flags _] (parse-flags args ["--home" "--output"])]
    (print-data (install/install-home! :home (:home flags))
                :output (:output flags))))

(defn cmd-install-auth [args]
  (let [[flags clean-args] (parse-flags args ["--token" "--home" "--output"])
        clean-args (remove #(= "--apply" %) clean-args)
        apply (boolean (flag-present? args "--apply"))
        token (or (:token flags)
                  (first clean-args)
                  (.-FIGMA_TOKEN js/process.env)
                  (.-FIGMA_API_KEY js/process.env)
                  (when (and apply (not (.-isTTY js/process.stdin)))
                    (read-stdin)))]
    (print-data (install/install-auth! :token token
                                       :home (:home flags)
                                       :apply apply)
                :output (:output flags))))

(defn cmd-install-project [args]
  (let [[flags _] (parse-flags args ["--project-dir" "--output"])]
    (print-data (install/install-project! :project-dir (:project_dir flags))
                :output (:output flags))))

(defn cmd-install-skill [args]
  (let [[flags _] (parse-flags args ["--dir" "--home" "--client" "--clients" "--output"])
        apply (boolean (flag-present? args "--apply"))]
    (print-data (install/install-skill! :dir (:dir flags)
                                        :home (:home flags)
                                        :client (:client flags)
                                        :clients (:clients flags)
                                        :apply apply)
                :output (:output flags))))

(defn cmd-install-client [args]
  (let [[flags _] (parse-flags args ["--client" "--dir" "--transport" "--port"
                                      "--command" "--home" "--output"])
        apply (boolean (flag-present? args "--apply"))
        port (optional-int (:port flags))]
    (print-data (install/install-client! :client (:client flags)
                                         :dir (:dir flags)
                                         :transport (or (:transport flags) "http")
                                         :port (or port 9449)
                                         :command (or (:command flags) "fighorse")
                                         :home (:home flags)
                                         :apply apply)
                :output (:output flags))))

(defn cmd-install-service [args]
  (let [[flags _] (parse-flags args ["--service" "--port" "--command" "--home" "--output"])
        apply (boolean (flag-present? args "--apply"))
        port (optional-int (:port flags))]
    (print-data (install/install-service! :service (or (:service flags) "auto")
                                          :port (or port 9449)
                                          :command (or (:command flags) "fighorse")
                                          :home (:home flags)
                                          :apply apply)
                :output (:output flags))))

(defn cmd-install-binary [args]
  (let [[flags _] (parse-flags args ["--source" "--target" "--link-dir" "--link-dirs" "--home" "--output"])
        apply (boolean (flag-present? args "--apply"))]
    (print-data (install/install-binary! :source (:source flags)
                                         :target (:target flags)
                                         :link-dir (:link_dir flags)
                                         :link-dirs (:link_dirs flags)
                                         :home (:home flags)
                                         :apply apply)
                :output (:output flags))))

(defn cmd-install-all [args]
  (let [[flags _] (parse-flags args ["--client" "--clients" "--transport" "--port" "--command"
                                      "--home" "--project-dir" "--source" "--target" "--link-dir"
                                      "--link-dirs" "--service" "--token" "--mode" "--output"])
        apply (boolean (flag-present? args "--apply"))
        no-service (boolean (flag-present? args "--no-service"))
        port (optional-int (:port flags))]
    (print-data (install/install-all! :client (or (:client flags) "generic")
                                      :clients (:clients flags)
                                      :transport (or (:transport flags) "http")
                                      :port (or port 9449)
                                      :command (or (:command flags) "fighorse")
                                      :home (:home flags)
                                      :project-dir (:project_dir flags)
                                      :source (:source flags)
                                      :target (:target flags)
                                      :link-dir (:link_dir flags)
                                      :link-dirs (:link_dirs flags)
                                      :service (or (:service flags) "auto")
                                      :token (:token flags)
                                      :mode (:mode flags)
                                      :no-service no-service
                                      :apply apply)
                :output (:output flags))))

(defn cmd-install-status [args]
  (let [[flags _] (parse-flags args ["--output"])]
    (print-data (install/status) :output (:output flags))))

;; --- Auth commands ---

(defn cmd-discover [args]
  (let [[flags _] (parse-flags args ["--format" "--output"])
        manifest (discovery/manifest)
        format (or (:format flags) "json")]
    (if (= "md" format)
      (write-output! (discovery/manifest->markdown manifest) (:output flags))
      (print-data manifest :output (:output flags)))))

(defn cmd-quickstart [args]
  (let [[flags clean-args] (parse-flags args ["--format" "--output" "--figma-url"])
        figma-url (or (:figma_url flags) (first clean-args))
        report (discovery/quickstart :figma-url figma-url)
        format (or (:format flags) "md")]
    (if (= "json" format)
      (print-data report :output (:output flags))
      (write-output! (discovery/quickstart->markdown report) (:output flags)))))

(defn cmd-doctor [args]
  (let [[flags _] (parse-flags args ["--output"])]
    (print-data (discovery/doctor) :output (:output flags))))

(defn cmd-url-parse [args]
  (let [[flags clean-args] (parse-flags args ["--output"])
        input (require-arg clean-args 0 "figma-url")]
    (print-data (figma-url/parse-figma-url input) :output (:output flags))))

(defn cmd-mcp-config [args]
  (let [[flags _] (parse-flags args ["--client" "--transport" "--port" "--command" "--output"])
        port (or (optional-int (:port flags)) 9449)]
    (print-data (discovery/mcp-config :client (or (:client flags) "generic")
                                      :transport (or (:transport flags) "http")
                                      :port port
                                      :command (or (:command flags) "fighorse"))
                :output (:output flags))))

(defn cmd-figma-api-coverage [args]
  (let [[flags _] (parse-flags args ["--format" "--output"])
        report (api-coverage/coverage-report)
        format (or (:format flags) "json")]
    (if (= "md" format)
      (write-output! (api-coverage/coverage-report->markdown report) (:output flags))
      (print-data report :output (:output flags)))))

(defn- json-file! [file label]
  (parse-json-map! (.readFileSync fs file "utf8") label))

(defn cmd-figma-api-call [args]
  (let [token (require-token!)
        [flags clean-args] (parse-flags args ["--params" "--body" "--body-file" "--output"])
        operation-id (require-arg clean-args 0 "operation-id")
        params (or (parse-json-map! (:params flags) "--params") {})
        body (or (parse-json-map! (:body flags) "--body")
                 (when (:body_file flags)
                   (json-file! (:body_file flags) "--body-file"))
                 {})
        explain? (flag-present? args "--explain-for-ai")]
    (when (and (api-operations/write-operation? operation-id)
               (not (flag-present? args "--yes")))
      (eprintln "Error: write operation requires --yes. Figma write APIs can mutate comments, variables, webhooks, or dev resources.")
      (js/process.exit 1))
    (-> (api-operations/call-operation token operation-id params body)
        (.then (fn [data]
                 (print-data (if explain?
                               (api-operations/result-envelope operation-id data)
                               data)
                             :output (:output flags))))
        (.catch (fn [err]
                  (eprintln "Error:" (or (.-message err) (str err)))
                  (js/process.exit 1))))))

(defn cmd-visual-audit [args]
  (let [[flags clean-args] (parse-flags args ["--screenshot" "--platform" "--asset-format" "--notes" "--output"])
        figma-url (require-arg clean-args 0 "figma-url")]
    (print-data (visual-audit/audit :figma-url figma-url
                                    :screenshot-path (:screenshot flags)
                                    :platform (:platform flags)
                                    :asset-format (:asset_format flags)
                                    :notes (:notes flags))
                :output (:output flags))))

(defn cmd-project-playbook [args]
  (let [[flags _] (parse-flags args ["--platform" "--asset-format" "--project-dir" "--output"])]
    (print-data (playbook/build :platform (:platform flags)
                                :asset-format (:asset_format flags)
                                :project-dir (:project_dir flags))
                :output (:output flags))))

(defn cmd-design-package [args]
  (let [token (require-token!)
        [flags clean-args] (parse-flags args ["--node-id" "--depth" "--max-tokens" "--output"
                                              "--screenshot-format" "--scale" "--screenshot-limit"
                                              "--platform" "--asset-format"])
        input (require-arg clean-args 0 "figma-url-or-file-key")
        depth (optional-int (:depth flags))
        max-tokens (optional-int (:max_tokens flags))
        include-screenshot (not (flag-present? args "--no-screenshot"))
        include-assets (flag-present? args "--include-assets")
        scale (optional-float (:scale flags))
        screenshot-limit (optional-int (:screenshot_limit flags))]
    (-> (design-package/get-design-package token
                                           :figma-url input
                                           :node-id (:node_id flags)
                                           :depth (or depth 2)
                                           :max-tokens (or max-tokens 8000)
                                           :include-screenshot include-screenshot
                                           :include-assets include-assets
                                           :screenshot-format (or (:screenshot_format flags) "png")
                                           :scale (or scale 2)
                                           :screenshot-limit (or screenshot-limit 4)
                                           :platform (:platform flags)
                                           :asset-format (:asset_format flags))
        (.then (fn [data] (print-data data :output (:output flags))))
        (.catch (fn [err]
                  (eprintln "Error:" (or (.-message err) (str err)))
                  (js/process.exit 1))))))

(defn cmd-smoke [args]
  (let [token (require-token!)
        [flags clean-args] (parse-flags args ["--output"])
        input (require-arg clean-args 0 "figma-url-or-file-key")
        parsed (figma-url/parse-figma-url input)]
    (-> (design-package/get-design-package token
                                           :figma-url input
                                           :depth 1
                                           :max-tokens 3000
                                           :include-screenshot true
                                           :include-assets false)
        (.then (fn [pkg]
                 (print-data {:kind "fighorse.smoke.v1"
                              :ok (= "ready" (get-in pkg [:diagnostics :status]))
                              :parsed_input parsed
                              :source (:source pkg)
                              :file (:file pkg)
                              :target (:target pkg)
                              :diagnostics (:diagnostics pkg)
                              :next_steps (cond-> ["Use fighorse design package with explicit --platform and --asset-format for implementation context."]
                                            (not (:node_id parsed))
                                            (conj "Copy a link to a selected frame, component, or group so the URL includes node-id.")
                                            (= "CANVAS" (get-in pkg [:target :type]))
                                            (conj "Current target is a CANVAS/page; use screen_candidates from a design package to pick exact frames."))}
                             :output (:output flags))))
        (.catch (fn [err]
                  (print-data {:kind "fighorse.smoke.v1"
                               :ok false
                               :error (or (.-message err) (str err))
                               :parsed_input parsed
                               :checks [{:id "token"
                                         :next_step "Run fighorse auth login --token <FIGMA_TOKEN> or set FIGMA_TOKEN."}
                                        {:id "figma_url"
                                         :next_step "Use fighorse quickstart \"<figma-frame-url>\" to verify URL parsing before smoke."}
                                        {:id "proxy"
                                         :next_step "If your network requires a proxy, set HTTPS_PROXY or ALL_PROXY."}]
                               :next_step "Run fighorse doctor --format json and verify FIGMA_TOKEN, file permissions, proxy, and Figma URL."}
                              :output (:output flags))
                  (js/process.exit 1))))))

(defn cmd-auth-login [args]
  (let [[flags clean-args] (parse-flags args ["--token"])
        raw-token (or (:token flags)
                      (first clean-args)
                      (when-not (.-isTTY js/process.stdin)
                        (read-stdin)))
        token (when raw-token (str/trim raw-token))]
    (if (str/blank? token)
      (do
        (eprintln "Error: token required. Use `fighorse auth login --token <token>` or pipe token on stdin.")
        (js/process.exit 1))
      (do
        (config/save-config! {:token token})
        (println (str "Saved Figma token to " (config/config-path)))))))

(defn cmd-auth-logout [_args]
  (config/clear-config!)
  (println "Removed saved Figma token"))

(defn cmd-auth-status [_args]
  (let [{:keys [token config-path]} (config/load-config)]
    (if (str/blank? token)
      (println (str "Not authenticated. Config path: " config-path))
      (println (str "Authenticated. Config path: " config-path)))))

;; --- File commands ---

(defn cmd-file-get [args]
  (let [token (require-token!)
        [flags clean-args] (parse-flags args ["--version" "--depth" "--ids" "--geometry" "--output"])
        file-key (require-arg clean-args 0 "file-key")
        depth (or (optional-int (:depth flags))
                  (optional-int (nth clean-args 1 nil)))]
    (print-json (files-api/get-file token file-key
                                    :depth depth
                                    :ids (:ids flags)
                                    :geometry (:geometry flags)
                                    :version (:version flags))
                :output (:output flags))))

(defn cmd-file-nodes [args]
  (let [token (require-token!)
        file-key (require-arg args 0 "file-key")
        ids (require-arg args 1 "node-ids")
        [flags _] (parse-flags (drop 2 args) ["--depth"])
        depth (optional-int (:depth flags))]
    (print-json (files-api/get-file-nodes token file-key ids :depth depth))))

(defn cmd-file-meta [args]
  (let [token (require-token!)
        file-key (require-arg args 0 "file-key")]
    (print-json (files-api/get-file-meta token file-key))))

(defn cmd-file-versions [args]
  (let [token (require-token!)
        file-key (require-arg args 0 "file-key")
        [flags _] (parse-flags (rest args) ["--page-size"])
        page-size (optional-int (:page_size flags))]
    (print-json (files-api/get-file-versions token file-key :page-size page-size))))

(defn cmd-file-compact [args]
  (let [token (require-token!)
        [flags clean-args] (parse-flags args ["--max-tokens" "--depth" "--ids" "--output"])
        file-key (require-arg clean-args 0 "file-key")
        depth (or (optional-int (:depth flags))
                  (optional-int (nth clean-args 1 nil)))
        max-tokens (optional-int (:max_tokens flags))]
    (-> (files-api/get-file token file-key :depth depth :ids (:ids flags))
        (.then (fn [data]
                 (let [node (figma/response->node data)
                       compacted (compact/compact node
                                                   :extractors compact/default-extractors
                                                   :max-depth depth
                                                   :max-tokens max-tokens)]
                   (print-data compacted :output (:output flags)))))
        (.catch (fn [err]
                  (println "Error:" (or (.-message err) (str err)))
                  (js/process.exit 1))))))

(defn cmd-compact-stdin [args]
  (let [[flags _] (parse-flags args ["--max-tokens" "--depth" "--output"])
        input (js->clj (js/JSON.parse (read-stdin)) :keywordize-keys true)
        node (figma/response->node input)
        depth (optional-int (:depth flags))
        max-tokens (optional-int (:max_tokens flags))
        compacted (compact/compact node
                                   :extractors compact/default-extractors
                                   :max-depth depth
                                   :max-tokens max-tokens)]
    (print-data compacted :output (:output flags))))

(defn cmd-file-tree [args]
  (let [token (require-token!)
        [flags clean-args] (parse-flags args ["--depth" "--max-depth" "--output"])
        file-key (require-arg clean-args 0 "file-key")
        depth (or (optional-int (:depth flags))
                  (optional-int (:max_depth flags))
                  (optional-int (nth clean-args 1 nil)))]
    (-> (files-api/get-file token file-key :depth (or depth 2))
        (.then (fn [data]
                 (let [doc (get-in data [:document])
                       tree (compact/simplify-tree doc
                                                    [compact/dimension-extractor
                                                     compact/layout-extractor]
                                                    depth)]
                   (print-data tree :output (:output flags)))))
        (.catch (fn [err]
                  (println "Error:" (or (.-message err) (str err)))
                  (js/process.exit 1))))))

(defn cmd-file-to-md [args]
  (let [token (require-token!)
        [flags clean-args] (parse-flags args ["--title" "--depth" "--output"])
        file-key (require-arg clean-args 0 "file-key")
        depth (or (optional-int (:depth flags))
                  (optional-int (nth clean-args 1 nil)))
        include-tokens (flag-present? args "--include-tokens")
        include-screenshots (flag-present? args "--include-screenshots")]
    (-> (files-api/get-file token file-key :depth (or depth 2))
        (.then (fn [data]
                 (let [doc (get-in data [:document])
                       simplified (compact/simplify-tree doc compact/default-extractors depth)
                       md (md-export/tree->markdown simplified :title (or (:title flags) (:name doc)))
                       final-md (if include-tokens
                                  (str md
                                       "\n\n## Design Tokens\n\n```json\n"
                                       (json-str (tokens/tokens-by-category
                                                  (tokens/extract-tokens simplified)))
                                       "\n```")
                                  md)
                       screenshot-ids (->> (:children doc)
                                           (map :id)
                                           (remove str/blank?)
                                           (take 8)
                                           (str/join ","))]
                   (if (and include-screenshots (seq screenshot-ids))
                     (-> (files-api/get-images token file-key screenshot-ids :format "png" :scale 2)
                         (.then (fn [image-data]
                                  (let [images (:images image-data)
                                        screenshots (str "\n\n## Screenshots\n\n"
                                                         (str/join "\n"
                                                                   (map (fn [[id url]]
                                                                          (str "- [" (name id) "](" url ")"))
                                                                        images)))]
                                    (write-output! (str final-md screenshots) (:output flags))))))
                     (write-output! final-md (:output flags))))))
        (.catch (fn [err]
                  (println "Error:" (or (.-message err) (str err)))
                  (js/process.exit 1))))))

;; --- Image commands ---

(defn cmd-images-render [args]
  (let [token (require-token!)
        file-key (require-arg args 0 "file-key")
        ids (require-arg args 1 "node-ids")
        [flags _] (parse-flags (drop 2 args) ["--format" "--scale"])
        format (or (:format flags) "png")
        scale (or (optional-float (:scale flags)) 2)]
    (print-json (files-api/get-images token file-key ids :format format :scale scale))))

(defn cmd-images-fills [args]
  (let [token (require-token!)
        file-key (require-arg args 0 "file-key")]
    (print-json (files-api/get-image-fills token file-key))))

(defn cmd-images-export [args]
  (let [token (require-token!)
        file-key (require-arg args 0 "file-key")
        ids (require-arg args 1 "node-ids")
        [flags _] (parse-flags (drop 2 args) ["--format" "--scale" "--dir" "--prefix"])
        format (or (:format flags) "png")
        scale (or (optional-float (:scale flags)) 2)
        dir (:dir flags)
        manifest (flag-present? args "--manifest")]
    (-> (img-export/export-images token file-key (str/split ids #",")
          :format format :scale scale :dest-dir dir :manifest manifest :prefix (:prefix flags))
        (.then (fn [results]
                 (println "Exported images:")
                 (doseq [[id path] results]
                   (println (str "  " id " -> " path)))
                 (finish-cli!)))
        (.catch (fn [err]
                  (println "Error:" (or (.-message err) (str err)))
                  (js/process.exit 1))))))

(defn cmd-image-export [args]
  (let [[flags clean-args] (parse-flags args ["--ids"])
        file-key (require-arg clean-args 0 "file-key")
        ids (or (:ids flags) (require-arg clean-args 1 "node-ids"))
        remaining (if (:ids flags)
                    (drop 1 clean-args)
                    (drop 2 clean-args))]
    (cmd-images-export (into [file-key ids] remaining))))

(defn cmd-assets-download [args]
  (let [token (require-token!)
        file-key (require-arg args 0 "file-key")
        [flags _] (parse-flags (rest args) ["--dir" "--prefix"])
        dir (:dir flags)
        manifest (flag-present? args "--manifest")]
    (-> (img-export/download-image-fills token file-key
          :dest-dir dir :manifest manifest :prefix (:prefix flags))
        (.then (fn [results]
                 (println "Downloaded assets:")
                 (doseq [[id path] results]
                   (println (str "  " id " -> " path)))
                 (finish-cli!)))
        (.catch (fn [err]
                  (println "Error:" (or (.-message err) (str err)))
                  (js/process.exit 1))))))

;; --- Comment commands ---

(defn cmd-comments-list [args]
  (let [token (require-token!)
        file-key (require-arg args 0 "file-key")
        [flags _] (parse-flags (rest args) ["--as-md"])
        as-md (or (flag-present? args "--as-md")
                  (= "true" (:as_md flags)))]
    (print-json (comments-api/get-comments token file-key :as-md as-md))))

(defn cmd-comments-post [args]
  (let [token (require-token!)
        file-key (require-arg args 0 "file-key")
        message (require-arg args 1 "message")
        [flags _] (parse-flags (drop 2 args) ["--reply-to"])]
    (print-json (comments-api/post-comment token file-key message :comment-id (:reply_to flags)))))

(defn cmd-comments-delete [args]
  (let [token (require-token!)
        file-key (require-arg args 0 "file-key")
        comment-id (require-arg args 1 "comment-id")]
    (print-json (comments-api/delete-comment token file-key comment-id))))

;; --- Project commands ---

(defn cmd-projects-list [args]
  (let [token (require-token!)
        team-id (require-arg args 0 "team-id")]
    (print-json (projects-api/get-team-projects token team-id))))

(defn cmd-project-files [args]
  (let [token (require-token!)
        project-id (require-arg args 0 "project-id")]
    (print-json (projects-api/get-project-files token project-id))))

;; --- User commands ---

(defn cmd-me [args]
  (let [token (require-token!)]
    (print-json (users-api/get-me token))))

;; --- Component commands ---

(defn cmd-components-team [args]
  (let [token (require-token!)
        team-id (require-arg args 0 "team-id")
        [flags _] (parse-flags (rest args) ["--page-size"])
        page-size (optional-int (:page_size flags))]
    (print-json (components-api/get-team-components token team-id :page-size page-size))))

(defn cmd-components-file [args]
  (let [token (require-token!)
        file-key (require-arg args 0 "file-key")]
    (print-json (components-api/get-file-components token file-key))))

(defn cmd-component-get [args]
  (let [token (require-token!)
        component-key (require-arg args 0 "component-key")]
    (print-json (components-api/get-component token component-key))))

(defn cmd-component-sets-file [args]
  (let [token (require-token!)
        file-key (require-arg args 0 "file-key")]
    (print-json (components-api/get-file-component-sets token file-key))))

(defn cmd-components-list [args]
  (let [token (require-token!)
        [flags clean-args] (parse-flags args ["--team" "--page-size"])
        page-size (optional-int (:page_size flags))]
    (if (:team flags)
      (print-json (components-api/get-team-components token (:team flags) :page-size page-size))
      (let [file-key (require-arg clean-args 0 "file-key")]
        (print-json (components-api/get-file-components token file-key))))))

;; --- Style commands ---

(defn cmd-styles-team [args]
  (let [token (require-token!)
        team-id (require-arg args 0 "team-id")
        [flags _] (parse-flags (rest args) ["--page-size"])
        page-size (optional-int (:page_size flags))]
    (print-json (styles-api/get-team-styles token team-id :page-size page-size))))

(defn cmd-styles-file [args]
  (let [token (require-token!)
        file-key (require-arg args 0 "file-key")]
    (print-json (styles-api/get-file-styles token file-key))))

(defn cmd-style-get [args]
  (let [token (require-token!)
        style-key (require-arg args 0 "style-key")]
    (print-json (styles-api/get-style token style-key))))

(defn cmd-styles-list [args]
  (let [token (require-token!)
        [flags clean-args] (parse-flags args ["--team" "--page-size"])
        page-size (optional-int (:page_size flags))]
    (if (:team flags)
      (print-json (styles-api/get-team-styles token (:team flags) :page-size page-size))
      (let [file-key (require-arg clean-args 0 "file-key")]
        (print-json (styles-api/get-file-styles token file-key))))))

;; --- Variable commands ---

(defn cmd-variables-local [args]
  (let [token (require-token!)
        file-key (require-arg args 0 "file-key")]
    (print-json (variables-api/get-local-variables token file-key))))

(defn cmd-variables-published [args]
  (let [token (require-token!)
        file-key (require-arg args 0 "file-key")]
    (print-json (variables-api/get-published-variables token file-key))))

(defn cmd-variables-list [args]
  (cmd-variables-local args))

;; --- Dev resource commands ---

(defn cmd-dev-resources-list [args]
  (let [token (require-token!)
        file-key (require-arg args 0 "file-key")]
    (print-json (dev-resources-api/get-dev-resources token file-key))))

;; --- Webhook commands ---

(defn cmd-webhooks-list [args]
  (let [token (require-token!)
        [flags _] (parse-flags args ["--context"])]
    (print-json (webhooks-api/get-webhooks token :context (:context flags)))))

(defn cmd-webhooks-create [args]
  (let [token (require-token!)
        event-type (require-arg args 0 "event-type")
        team-id (require-arg args 1 "team-id")
        endpoint (require-arg args 2 "endpoint")]
    (print-json (webhooks-api/create-webhook token event-type team-id endpoint))))

(defn cmd-webhooks-delete [args]
  (let [token (require-token!)
        webhook-id (require-arg args 0 "webhook-id")]
    (print-json (webhooks-api/delete-webhook token webhook-id))))

;; --- Transform commands ---

(defn cmd-file-filter [args]
  (let [[flags _] (parse-flags args ["--type" "--name-regex" "--min-size"])
        visible-only (flag-present? args "--visible-only")
        has-fill (flag-present? args "--has-fill")
        no-children (flag-present? args "--no-children")
        input (js->clj (js/JSON.parse (read-stdin)) :keywordize-keys true)
        filtered (tree-filter/filter-tree input
                                          {:types (tree-filter/parse-types (:type flags))
                                           :name-regex (:name_regex flags)
                                           :visible-only visible-only
                                           :min-size (tree-filter/parse-size (:min_size flags))
                                           :has-fill has-fill
                                           :no-children no-children})]
    (print-data filtered)))

(defn cmd-file-diff [args]
  (let [token (require-token!)
        [flags clean-args] (parse-flags args ["--from" "--to" "--depth" "--output"])
        file-key (require-arg clean-args 0 "file-key")
        depth (optional-int (:depth flags))
        from-version (require-value (or (:from flags) (nth clean-args 1 nil)) "--from")
        to-version (require-value (or (:to flags) (nth clean-args 2 nil)) "--to")]
    (-> (js/Promise.all
         #js [(files-api/get-file token file-key :version from-version :depth depth)
              (files-api/get-file token file-key :version to-version :depth depth)])
        (.then (fn [results]
                 (let [old-doc (get-in (aget results 0) [:document])
                       new-doc (get-in (aget results 1) [:document])]
                   (print-data (diff/diff-trees old-doc new-doc) :output (:output flags)))))
        (.catch (fn [err]
                  (eprintln "Error:" (or (.-message err) (str err)))
                  (js/process.exit 1))))))

(defn cmd-file-schema [args]
  (let [token (require-token!)
        [flags clean-args] (parse-flags args ["--component" "--depth" "--format" "--output"])
        file-key (require-arg clean-args 0 "file-key")
        component-id (require-value (or (:component flags) (nth clean-args 1 nil)) "--component")
        depth (optional-int (:depth flags))
        format (or (:format flags) "json")]
    (-> (files-api/get-file token file-key :depth depth)
        (.then (fn [data]
                 (let [doc (get-in data [:document])
                       inferred (schema/infer-component-schema doc component-id)]
                   (if inferred
                     (if (= "ts" format)
                       (write-output! (schema/schema->typescript inferred) (:output flags))
                       (print-data inferred :output (:output flags)))
                     (do
                       (eprintln "Error: component not found")
                       (js/process.exit 1))))))
        (.catch (fn [err]
                  (eprintln "Error:" (or (.-message err) (str err)))
                  (js/process.exit 1))))))

;; --- Token extraction ---

(defn cmd-tokens-extract [args]
  (let [token (require-token!)
        [flags clean-args] (parse-flags args ["--depth" "--format" "--prefix" "--output" "--category"])
        file-key (require-arg clean-args 0 "file-key")
        depth (or (optional-int (:depth flags))
                  (optional-int (nth clean-args 1 nil)))
        format (or (:format flags) "json")
        prefix (or (:prefix flags) "--figma-")
        category (:category flags)]
    (-> (files-api/get-file token file-key :depth (or depth 2))
        (.then (fn [data]
                 (let [doc (get-in data [:document])
                       simplified (compact/simplify-tree doc compact/default-extractors depth)
                       extracted (tokens/extract-tokens simplified)
                       selected (if (and category (not= "all" category))
                                  (filter #(= category (:type %)) extracted)
                                  extracted)
                       formatted (tokens/format-tokens selected :format format :prefix prefix)]
                   (if (string? formatted)
                     (write-output! formatted (:output flags))
                     (print-data (if (= "json" format)
                                   (tokens/tokens-by-category formatted)
                                   formatted)
                                 :output (:output flags))))))
        (.catch (fn [err]
                  (println "Error:" (or (.-message err) (str err)))
                  (js/process.exit 1))))))

;; --- MCP ---

(defn cmd-mcp-serve [args]
  (let [[flags _] (parse-flags args ["--transport" "--port" "--host" "--cors-origin"])
        transport (or (:transport flags) "sse")
        port (or (optional-int (:port flags)) 9449)]
    (mcp/serve :transport transport
               :port port
               :host (:host flags)
               :cors-origin (:cors_origin flags))))

;; --- Help ---

(defn cmd-help []
  (println "fighorse — Figma data Swiss Army knife, shaped for AI consumption")
  (println "")
  (println "Usage: fighorse <command> [args...]")
  (println "")
  (println "Start here:")
  (println "  fighorse quickstart                         Guided first-run setup")
  (println "  fighorse quickstart \"<figma-frame-url>\"      Validate token, frame link, and next command")
  (println "  fighorse quickstart \"<figma-frame-url>\" --format json")
  (println "                                                Machine-readable setup check for AI clients")
  (println "")
  (println "Run quickstart first. It explains required Figma token setup, frame-link scope,")
  (println "design-package commands, and MCP client setup when needed.")
  (println "")
  (println "Self Discovery and AI Replication:")
  (println "  quickstart [figma-url] [--format json]        Guided first-run readiness check")
  (println "  discover [--format json|md]                  Describe capabilities for AI tools")
  (println "  doctor [--format json]                       Check runtime/auth readiness")
  (println "  smoke <figma-url>                            Verify real Figma access and design package readiness")
  (println "  url parse <figma-url>                        Parse file_key and node_id")
  (println "  design package <figma-url> [--platform P] [--asset-format F]  Build AI replication package")
  (println "  mcp config [--client C] [--transport T]      Emit MCP client config")
  (println "  figma-api coverage [--format json|md]        Report official Figma REST OpenAPI coverage")
  (println "  figma api <operationId> --params JSON [--body JSON|--body-file P] [--yes]  Call any covered REST operation")
  (println "  visual audit <figma-url> [--screenshot P]    Produce AI-ready visual fidelity audit guidance")
  (println "  project playbook [--platform P]              Produce project-level fighorse AI playbook")
  (println "")
  (println "Experience / Self Learning:")
  (println "  experience schema                            Show versioned JSONL record schema")
  (println "  experience summary [--platform P] [--asset-format F] [--scope global|project|merged]  Prompt-ready local lessons")
  (println "  experience list [--platform P] [--category C] [--tag T]  List stored lessons")
  (println "  experience add --summary S --lesson L [...]  Record a reusable Figma replication lesson")
  (println "  experience path                              Show local experience store path")
  (println "")
  (println "Install:")
  (println "  install home [--home D]                       Create ~/.fighorse directories")
  (println "  install auth [--token T] [--apply]            Persist local Figma token for CLI and MCP")
  (println "  install binary --source P [--apply]           Install CLI binary into fighorse home and PATH links")
  (println "  install project [--project-dir D]             Enable project-scoped .fighorse experience")
  (println "  install client --client cursor|codex|kimi|claude|opencode|openclaw|hermes-agent [--apply]  Generate or apply client MCP setup")
  (println "  install service [--service launchd|systemd] [--apply]  Generate or apply auto-start MCP SSE service")
  (println "  install skill [--dir D] [--clients C] [--apply]  Generate or apply fighorse skill/agent files")
  (println "  install [--default|--path D|--target P] [--mode cli|service] [--apply]  Self-install this binary and emit AI-readable install guidance")
  (println "  install self [--default|--path D|--target P] [--apply]  Same as install root command")
  (println "  install all [--mode cli|service|all] [--no-service] [--clients C] [--source P] [--apply]  Generate or apply setup; default mode is cli")
  (println "  install status                                Show install paths and detected state")
  (println "")
  (println "Auth:")
  (println "  auth login --token <token>                    Save Figma token")
  (println "  auth logout                                   Remove saved token")
  (println "  auth status                                   Show auth status")
  (println "")
  (println "File Operations:")
  (println "  file get <file-key> [depth] [--version V] [--output P]  Fetch raw file JSON")
  (println "  file nodes <file-key> <ids> [--depth N]       Fetch specific nodes")
  (println "  node get <file-key> <id> [--depth N]          Fetch a specific node")
  (println "  file meta <file-key>                          Get file metadata")
  (println "  file versions <file-key> [--page-size N]      Get version history")
  (println "  file compact <file-key> [depth] [--max-tokens N]  Smart truncation for AI")
  (println "  compact < input.json [--max-tokens N]       Compact JSON from stdin")
  (println "  file filter < input.json [--type T] [--visible-only]  Filter a tree")
  (println "  file diff <file-key> --from V1 --to V2        Diff two versions")
  (println "  file tree <file-key> [depth]                  Lightweight structure view")
  (println "  file to-md <file-key> [depth] [--title T]     Export as Markdown")
  (println "  file tokens <file-key> [--format json|css|scss|tailwind]  Extract tokens")
  (println "  file schema <file-key> --component ID [--format json|ts]  Infer component props")
  (println "")
  (println "Image Operations:")
  (println "  Recommended export dirs: ./.fighorse/exports for scratch, ./assets/fighorse for packaged assets")
  (println "  image export <file-key> --ids <ids> [--format F] [--scale S] [--dir D] [--manifest]  Download node slices")
  (println "  images render <file-key> <ids> [--format F] [--scale S]  Get render URLs")
  (println "  images fills <file-key>                        Get image fill URLs")
  (println "  images export <file-key> <ids> [--format F] [--scale S] [--dir D] [--manifest]  Download node slices")
  (println "  asset download <file-key> [--dir D] [--manifest]  Download all image fills")
  (println "")
  (println "Comments:")
  (println "  comments list <file-key> [--as-md]            List comments")
  (println "  comments post <file-key> <message> [--reply-to ID]  Post comment")
  (println "  comments delete <file-key> <comment-id>       Delete comment")
  (println "")
  (println "Projects:")
  (println "  projects list <team-id>                       List team projects")
  (println "  project files <project-id>                    List project files")
  (println "")
  (println "Users:")
  (println "  me                                            Get current user")
  (println "")
  (println "Components:")
  (println "  components list <file-key>                    List file components")
  (println "  components list --team <team-id> [--page-size N]  List team components")
  (println "  components team <team-id> [--page-size N]     List team components")
  (println "  components file <file-key>                    List file components")
  (println "  component get <component-key>                 Get component metadata")
  (println "  component export <file-key> --ids <ids> [--format F] [--dir D]  Export component/control nodes")
  (println "  component-sets file <file-key>                List file component sets")
  (println "")
  (println "Styles:")
  (println "  styles list <file-key>                        List file styles")
  (println "  styles list --team <team-id> [--page-size N]  List team styles")
  (println "  styles team <team-id> [--page-size N]         List team styles")
  (println "  styles file <file-key>                        List file styles")
  (println "  style get <style-key>                         Get style metadata")
  (println "")
  (println "Variables:")
  (println "  variables list <file-key>                     Get local variables")
  (println "  variables local <file-key>                    Get local variables")
  (println "  variables published <file-key>                Get published variables")
  (println "")
  (println "Dev Resources:")
  (println "  dev-resources list <file-key>                 List dev resources")
  (println "")
  (println "Webhooks:")
  (println "  webhooks list [--context C]                   List webhooks")
  (println "  webhooks create <event-type> <team-id> <endpoint>  Create webhook")
  (println "  webhooks delete <webhook-id>                  Delete webhook")
  (println "")
  (println "Design Tokens:")
  (println "  tokens extract <file-key> [depth]             Extract design tokens")
  (println "")
  (println "MCP Server:")
  (println "  mcp serve [--transport http|sse|stdio] [--port N] [--host 127.0.0.1]  Start MCP server")
  (println "")
  (println "Environment:")
  (println "  FIGMA_TOKEN    Figma Personal Access Token")
  (println "  FIGHORSE_HOME  Default: ~/.fighorse")
  (println "  FIGHORSE_MCP_MODE  MCP safety mode: readonly (default) or write")
  (println "  FIGHORSE_MCP_LOCAL_WRITE  Set to allow for MCP local asset exports inside approved roots")
  (println "  FIGHORSE_MCP_STDIO_MAX_BYTES  Max stdio Content-Length message size, default 10485760")
  (println "  FIGHORSE_MCP_ALLOW_MULTIPLE  Set to 1 only for development when bypassing the MCP singleton lock")
  (println "  FIGHORSE_HTTP_TIMEOUT_MS  Figma REST request timeout, default 120000")
  (println "  FIGHORSE_CLI_EXPLICIT_EXIT  Set to 0 only for tests/debugging to disable one-shot CLI explicit exit")
  (println "  FIGHORSE_EXPERIENCE_PATH  Override local experience JSONL store")
  (println "  FIGHORSE_EXPERIENCE_SCOPE  auto (default), global, project, or merged")
  (println "  HTTP_PROXY     HTTP proxy URL (e.g. http://127.0.0.1:7897)")
  (println "  HTTPS_PROXY    HTTPS proxy URL")
  (println "  ALL_PROXY      Fallback proxy URL")
  (println "")
  (println "Proxy Example:")
  (println "  HTTPS_PROXY=http://127.0.0.1:7897 fighorse file meta <file-key>"))

;; --- Entry point ---

(defn ^:export main []
  (let [config (config/load-config)
        _ (config/setup-proxy! (:proxy config))
        args (vec (drop 2 (js->clj js/process.argv)))
        cmd1 (first args)
        cmd2 (second args)
        cmd3 (nth args 2 nil)
        rest-args (drop 2 args)]
    (when-not (= [cmd1 cmd2] ["mcp" "serve"])
      (install-cli-signal-handlers!))
    (cond
      ;; Help
      (or (= cmd1 "help")
          (flag-present? args "--help")
          (flag-present? args "-h"))       (cmd-help)
      ;; Self discovery and AI replication
      (= [cmd1] ["quickstart"])            (cmd-quickstart (rest args))
      (= [cmd1] ["discover"])              (cmd-discover (rest args))
      (= [cmd1] ["doctor"])                (cmd-doctor (rest args))
      (= [cmd1] ["smoke"])                 (cmd-smoke (rest args))
      (= [cmd1 cmd2] ["url" "parse"])      (cmd-url-parse rest-args)
      (= [cmd1 cmd2] ["design" "package"]) (cmd-design-package rest-args)
      (= [cmd1 cmd2] ["mcp" "config"])     (cmd-mcp-config rest-args)
      (= [cmd1 cmd2] ["figma-api" "coverage"]) (cmd-figma-api-coverage rest-args)
      (= [cmd1 cmd2] ["figma" "api"])      (cmd-figma-api-call rest-args)
      (= [cmd1 cmd2] ["visual" "audit"])   (cmd-visual-audit rest-args)
      (= [cmd1 cmd2] ["project" "playbook"]) (cmd-project-playbook rest-args)
      (= [cmd1 cmd2] ["experience" "schema"]) (cmd-experience-schema rest-args)
      (= [cmd1 cmd2] ["experience" "summary"]) (cmd-experience-summary rest-args)
      (= [cmd1 cmd2] ["experience" "list"]) (cmd-experience-list rest-args)
      (= [cmd1 cmd2] ["experience" "add"]) (cmd-experience-add rest-args)
      (= [cmd1 cmd2] ["experience" "path"]) (cmd-experience-path rest-args)
      (and (= cmd1 "install")
           (or (nil? cmd2)
               (str/starts-with? cmd2 "--"))) (cmd-install-self (rest args))
      (= [cmd1 cmd2] ["install" "self"]) (cmd-install-self rest-args)
      (= [cmd1 cmd2] ["install" "home"]) (cmd-install-home rest-args)
      (= [cmd1 cmd2] ["install" "auth"]) (cmd-install-auth rest-args)
      (= [cmd1 cmd2] ["install" "project"]) (cmd-install-project rest-args)
      (= [cmd1 cmd2] ["install" "binary"]) (cmd-install-binary rest-args)
      (= [cmd1 cmd2] ["install" "client"]) (cmd-install-client rest-args)
      (= [cmd1 cmd2] ["install" "service"]) (cmd-install-service rest-args)
      (= [cmd1 cmd2] ["install" "skill"]) (cmd-install-skill rest-args)
      (= [cmd1 cmd2] ["install" "all"]) (cmd-install-all rest-args)
      (= [cmd1 cmd2] ["install" "status"]) (cmd-install-status rest-args)
      (= [cmd1] ["install"]) (cmd-install-all (rest args))
      ;; Auth
      (= [cmd1 cmd2] ["auth" "login"])    (cmd-auth-login rest-args)
      (= [cmd1 cmd2] ["auth" "logout"])   (cmd-auth-logout rest-args)
      (= [cmd1 cmd2] ["auth" "status"])   (cmd-auth-status rest-args)
      ;; File commands
      (= [cmd1 cmd2] ["file" "get"])       (cmd-file-get rest-args)
      (= [cmd1 cmd2] ["file" "nodes"])     (cmd-file-nodes rest-args)
      (= [cmd1 cmd2] ["file" "meta"])      (cmd-file-meta rest-args)
      (= [cmd1 cmd2] ["file" "versions"])  (cmd-file-versions rest-args)
      (= [cmd1 cmd2] ["file" "compact"])   (cmd-file-compact rest-args)
      (= [cmd1 cmd2] ["file" "filter"])    (cmd-file-filter rest-args)
      (= [cmd1 cmd2] ["file" "diff"])      (cmd-file-diff rest-args)
      (= [cmd1 cmd2] ["file" "tree"])      (cmd-file-tree rest-args)
      (= [cmd1 cmd2] ["file" "to-md"])     (cmd-file-to-md rest-args)
      (= [cmd1 cmd2] ["file" "tokens"])    (cmd-tokens-extract rest-args)
      (= [cmd1 cmd2] ["file" "schema"])    (cmd-file-schema rest-args)
      (= [cmd1] ["compact"])               (cmd-compact-stdin (rest args))
      ;; Node alias
      (= [cmd1 cmd2] ["node" "get"])       (cmd-file-nodes rest-args)
      ;; Image commands
      (= [cmd1 cmd2] ["image" "export"])   (cmd-image-export rest-args)
      (= [cmd1 cmd2] ["images" "render"])  (cmd-images-render rest-args)
      (= [cmd1 cmd2] ["images" "fills"])   (cmd-images-fills rest-args)
      (= [cmd1 cmd2] ["images" "export"])  (cmd-images-export rest-args)
      (= [cmd1 cmd2] ["asset" "download"]) (cmd-assets-download rest-args)
      ;; Comment commands
      (= [cmd1 cmd2] ["comments" "list"])  (cmd-comments-list rest-args)
      (= [cmd1 cmd2] ["comments" "post"])  (cmd-comments-post rest-args)
      (= [cmd1 cmd2] ["comments" "delete"]) (cmd-comments-delete rest-args)
      ;; Project commands
      (= [cmd1 cmd2] ["projects" "list"])  (cmd-projects-list rest-args)
      (= [cmd1 cmd2] ["project" "files"])  (cmd-project-files rest-args)
      ;; User commands
      (= [cmd1] ["me"])                    (cmd-me rest-args)
      ;; Component commands
      (= [cmd1 cmd2] ["components" "list"]) (cmd-components-list rest-args)
      (= [cmd1 cmd2] ["components" "team"]) (cmd-components-team rest-args)
      (= [cmd1 cmd2] ["components" "file"]) (cmd-components-file rest-args)
      (= [cmd1 cmd2] ["component" "get"])  (cmd-component-get rest-args)
      (= [cmd1 cmd2] ["component" "export"]) (cmd-image-export rest-args)
      (= [cmd1 cmd2] ["component-sets" "file"]) (cmd-component-sets-file rest-args)
      ;; Style commands
      (= [cmd1 cmd2] ["styles" "list"])    (cmd-styles-list rest-args)
      (= [cmd1 cmd2] ["styles" "team"])    (cmd-styles-team rest-args)
      (= [cmd1 cmd2] ["styles" "file"])    (cmd-styles-file rest-args)
      (= [cmd1 cmd2] ["style" "get"])      (cmd-style-get rest-args)
      ;; Variable commands
      (= [cmd1 cmd2] ["variables" "list"])      (cmd-variables-list rest-args)
      (= [cmd1 cmd2] ["variables" "local"])     (cmd-variables-local rest-args)
      (= [cmd1 cmd2] ["variables" "published"]) (cmd-variables-published rest-args)
      ;; Dev resource commands
      (= [cmd1 cmd2] ["dev-resources" "list"])  (cmd-dev-resources-list rest-args)
      ;; Webhook commands
      (= [cmd1 cmd2] ["webhooks" "list"])       (cmd-webhooks-list rest-args)
      (= [cmd1 cmd2] ["webhooks" "create"])     (cmd-webhooks-create rest-args)
      (= [cmd1 cmd2] ["webhooks" "delete"])     (cmd-webhooks-delete rest-args)
      ;; Token extraction
      (= [cmd1 cmd2] ["tokens" "extract"])      (cmd-tokens-extract rest-args)
      ;; MCP
      (= [cmd1] ["mcp"])                   (cmd-mcp-serve rest-args)
      :else                                (cmd-help))))

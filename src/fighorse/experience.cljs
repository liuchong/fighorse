(ns fighorse.experience
  "Local, versioned experience store for AI-assisted Figma replication."
  (:require [clojure.string :as str]
            [fighorse.config :as config]
            [fighorse.guidance :as guidance]))

(def ^:private fs (js/require "fs"))
(def ^:private path (js/require "path"))
(def ^:private os (js/require "os"))

(def record-kind "fighorse.experience.v1")
(def schema-version 1)
(def summary-kind "fighorse.experience-summary.v1")
(def guidance-kind "fighorse.learned-guidance.v1")

(defn global-experience-path []
  (.join path (config/fighorse-home) "experience" "global.jsonl"))

(defn resolve-project-dir
  "Resolve the current fighorse project directory. Falls back to cwd."
  [& {:keys [project-dir]}]
  (or project-dir
      (.-FIGHORSE_PROJECT_DIR js/process.env)
      (loop [dir (.cwd js/process)]
        (let [project-config (.join path dir ".fighorse" "fighorse.json")
              git-dir (.join path dir ".git")
              parent (.dirname path dir)]
          (cond
            (.existsSync fs project-config) dir
            (.existsSync fs git-dir) dir
            (= parent dir) (.cwd js/process)
            :else (recur parent))))))

(defn project-config-path [& {:keys [project-dir]}]
  (.join path (resolve-project-dir :project-dir project-dir) ".fighorse" "fighorse.json"))

(defn project-experience-path [& {:keys [project-dir]}]
  (.join path (resolve-project-dir :project-dir project-dir) ".fighorse" "experience.jsonl"))

(defn project-installed? [& {:keys [project-dir]}]
  (.existsSync fs (project-config-path :project-dir project-dir)))

(defn- requested-scope [scope]
  (let [value (some-> (or scope
                          (.-FIGHORSE_EXPERIENCE_SCOPE js/process.env)
                          "auto")
                      str
                      str/trim
                      str/lower-case)]
    (if (str/blank? value) "auto" value)))

(defn effective-scope [& {:keys [scope project-dir]}]
  (let [scope (requested-scope scope)]
    (cond
      (.-FIGHORSE_EXPERIENCE_PATH js/process.env) "explicit"
      (= "auto" scope) (if (project-installed? :project-dir project-dir) "project" "global")
      (#{"global" "project" "merged"} scope) scope
      :else "global")))

(defn experience-path
  "Return the write path for experience records."
  [& {:keys [scope project-dir]}]
  (or (.-FIGHORSE_EXPERIENCE_PATH js/process.env)
      (case (effective-scope :scope scope :project-dir project-dir)
        "project" (project-experience-path :project-dir project-dir)
        "merged" (if (project-installed? :project-dir project-dir)
                   (project-experience-path :project-dir project-dir)
                   (global-experience-path))
        (global-experience-path))))

(defn experience-read-paths
  [& {:keys [scope project-dir]}]
  (if-let [explicit (.-FIGHORSE_EXPERIENCE_PATH js/process.env)]
    [explicit]
    (let [scope (effective-scope :scope scope :project-dir project-dir)
          project-path (project-experience-path :project-dir project-dir)
          global-path (global-experience-path)
          paths (case scope
                  "project" [project-path global-path]
                  "merged" [project-path global-path]
                  [global-path])]
      (vec (distinct paths)))))

(defn store-info [& {:keys [scope project-dir]}]
  {:home (config/fighorse-home)
   :scope (effective-scope :scope scope :project-dir project-dir)
   :project_dir (resolve-project-dir :project-dir project-dir)
   :project_config (project-config-path :project-dir project-dir)
   :project_installed (project-installed? :project-dir project-dir)
   :write_path (experience-path :scope scope :project-dir project-dir)
   :read_paths (experience-read-paths :scope scope :project-dir project-dir)
   :env_overrides (not-empty
                   (cond-> {}
                     (.-FIGHORSE_HOME js/process.env)
                     (assoc :FIGHORSE_HOME (.-FIGHORSE_HOME js/process.env))
                     (.-FIGHORSE_EXPERIENCE_PATH js/process.env)
                     (assoc :FIGHORSE_EXPERIENCE_PATH (.-FIGHORSE_EXPERIENCE_PATH js/process.env))
                     (.-FIGHORSE_EXPERIENCE_SCOPE js/process.env)
                     (assoc :FIGHORSE_EXPERIENCE_SCOPE (.-FIGHORSE_EXPERIENCE_SCOPE js/process.env))
                     (.-FIGHORSE_PROJECT_DIR js/process.env)
                     (assoc :FIGHORSE_PROJECT_DIR (.-FIGHORSE_PROJECT_DIR js/process.env))))})

(defn- now-iso []
  (.toISOString (js/Date.)))

(defn- random-id []
  (let [crypto (.-crypto js/globalThis)
        random-uuid (when crypto (.-randomUUID crypto))]
    (if random-uuid
      (.call random-uuid crypto)
      (str (.now js/Date) "-" (rand-int 1000000)))))

(defn- blank->nil [value]
  (let [value (some-> value str str/trim)]
    (when-not (str/blank? value)
      value)))

(defn- normalize-scalar [value default]
  (if-let [value (blank->nil value)]
    (str/lower-case value)
    default))

(defn- clean-map [m]
  (not-empty
   (into {}
         (keep (fn [[k v]]
                 (cond
                   (nil? v) nil
                   (and (string? v) (str/blank? v)) nil
                   (and (coll? v) (empty? v)) nil
                   :else [k v])))
         m)))

(defn- normalize-tags [tags]
  (let [values (cond
                 (string? tags) (str/split tags #",")
                 (sequential? tags) tags
                 :else [])]
    (not-empty
     (into []
           (comp (map blank->nil)
                 (remove nil?)
                 (map str/lower-case)
                 (distinct))
           values))))

(defn schema []
  {:kind "fighorse.experience-schema.v1"
   :record_kind record-kind
   :schema_version schema-version
   :store {:format "jsonl"
           :default_home (config/fighorse-home)
           :global_path (global-experience-path)
           :project_path (project-experience-path)
           :write_path (experience-path)
           :read_paths (experience-read-paths)
           :rules ["FIGHORSE_EXPERIENCE_PATH is an exact override."
                   "Global experience writes to ~/.fighorse/experience/global.jsonl by default."
                   "Run fighorse install project to enable project experience at ./.fighorse/experience.jsonl."
                   "When a project is installed, reads include project experience first and global experience second."
                   "Use FIGHORSE_EXPERIENCE_SCOPE=global|project|merged to override automatic scope."]
           :append_only true}
   :compatibility
   {:rule "New fields may be added. Readers must ignore unknown fields. Required v1 fields remain stable."
    :required_fields ["kind" "schema_version" "id" "created_at" "summary" "lesson"]
    :stable_fields ["source" "target" "category" "severity" "summary" "lesson" "recommendation" "evidence" "tags" "applies_to" "tool_context"]}
   :fields
   {:summary "Short problem or insight title. Required."
    :lesson "Reusable lesson learned. Required."
    :category "layout|typography|asset-export|platform|workflow|debugging|mcp|cli|other"
    :severity "info|warning|critical"
    :source {:figma_url "Optional Figma URL"
             :file_key "Optional Figma file key"
             :node_id "Optional Figma node id"}
    :target {:platform "Optional target platform/framework, e.g. android-compose, ios-swiftui, web-react, flutter"
             :asset_format "Optional export format, e.g. png/svg/pdf/webp"}
    :recommendation "Action AI should take next time."
    :evidence "What happened: screenshot diff, build error, overlap, etc."
    :tags "Comma-separated string or array."
    :tool_context {:client "cursor|codex|kimi-cli|opencode|other"
                   :command "CLI command or MCP tool that surfaced the issue"}}})

(defn normalize-record [input]
  (let [source (:source input)
        target (:target input)
        tool-context (:tool_context input)
        summary (blank->nil (:summary input))
        lesson (blank->nil (:lesson input))
        timestamp (now-iso)]
    (when-not summary
      (throw (js/Error. "experience.summary is required")))
    (when-not lesson
      (throw (js/Error. "experience.lesson is required")))
    (clean-map
     {:kind record-kind
      :schema_version schema-version
      :id (or (blank->nil (:id input)) (random-id))
      :created_at (or (blank->nil (:created_at input)) timestamp)
      :updated_at (or (blank->nil (:updated_at input)) timestamp)
      :category (normalize-scalar (:category input) "workflow")
      :severity (normalize-scalar (:severity input) "info")
      :source (clean-map {:figma_url (or (:figma_url input) (:figma-url input) (:figma_url source) (:figma-url source))
                          :file_key (or (:file_key input) (:file-key input) (:file_key source) (:file-key source))
                          :node_id (or (:node_id input) (:node-id input) (:node_id source) (:node-id source))})
      :target (clean-map {:platform (normalize-scalar (or (:platform input) (:platform target)) nil)
                          :asset_format (normalize-scalar (or (:asset_format input) (:asset-format input) (:asset_format target) (:asset-format target)) nil)})
      :summary summary
      :lesson lesson
      :recommendation (blank->nil (:recommendation input))
      :evidence (blank->nil (:evidence input))
      :tags (normalize-tags (:tags input))
      :applies_to (:applies_to input)
      :tool_context (clean-map {:client (or (:client input) (:client tool-context))
                                :command (or (:command input) (:command tool-context))})})))

(defn add!
  ([input] (add! input nil))
  ([input opts]
   (let [record (normalize-record input)
         store (experience-path :scope (:scope opts)
                                :project-dir (:project-dir opts))]
     (.mkdirSync fs (.dirname path store) #js {:recursive true})
     (.appendFileSync fs store (str (js/JSON.stringify (clj->js record)) "\n"))
     {:kind "fighorse.experience-write.v1"
      :store_path store
      :store (store-info :scope (:scope opts)
                         :project-dir (:project-dir opts))
      :record record
      :next_step "Call list_experiences or fighorse experience summary before the next Figma replication task."})))

(defn- parse-line [line]
  (when-not (str/blank? line)
    (try
      (js->clj (js/JSON.parse line) :keywordize-keys true)
      (catch :default _
        nil))))

(defn- migrate-record [record]
  (when record
    (try
      (normalize-record record)
      (catch :default _
        nil))))

(defn- read-store [store]
  (if (.existsSync fs store)
    (into []
          (keep (fn [line]
                  (when-let [record (migrate-record (parse-line line))]
                    (assoc record :_store_path store))))
          (str/split (.readFileSync fs store "utf8") #"\r?\n"))
    []))

(defn read-all
  ([] (read-all nil))
  ([opts]
   (mapv #(dissoc % :_store_path)
         (mapcat (comp reverse read-store)
                 (experience-read-paths :scope (:scope opts)
                                        :project-dir (:project-dir opts))))))

(defn- matches-text? [needle haystack]
  (let [needle (normalize-scalar needle nil)
        haystack (normalize-scalar haystack nil)]
    (or (nil? needle)
        (nil? haystack)
        (= "unspecified" haystack)
        (= needle haystack)
        (str/includes? haystack needle)
        (str/includes? needle haystack))))

(defn- matches? [record {:keys [platform asset-format category tag]}]
  (and (matches-text? platform (get-in record [:target :platform]))
       (matches-text? asset-format (get-in record [:target :asset_format]))
       (matches-text? category (:category record))
       (or (nil? (blank->nil tag))
           (contains? (set (:tags record)) (str/lower-case (str tag))))))

(defn compact-record [record]
  (select-keys record [:id :created_at :category :severity :source :target
                       :summary :lesson :recommendation :evidence :tags
                       :tool_context]))

(defn list-experiences
  [& {:keys [platform asset-format category tag limit scope project-dir]
      :or {limit 8}}]
  (let [opts {:scope scope :project-dir project-dir}
        records (read-all opts)
        limit (or limit 8)
        filtered (->> records
                      (filter #(matches? % {:platform platform
                                            :asset-format asset-format
                                            :category category
                                            :tag tag}))
                      (take limit)
                      (mapv compact-record))]
    {:kind summary-kind
     :schema_version schema-version
     :store_path (experience-path :scope scope :project-dir project-dir)
     :store (store-info :scope scope :project-dir project-dir)
     :total_count (count records)
     :returned_count (count filtered)
     :filters (clean-map {:platform platform
                          :asset_format asset-format
                          :category category
                          :tag tag
                          :scope scope
                          :project_dir project-dir
                          :limit limit})
     :records filtered}))

(defn guidance
  [& {:keys [platform asset-format category tag limit scope project-dir]
      :or {limit 6}}]
  (let [summary (list-experiences :platform platform
                                  :asset-format asset-format
                                  :category category
                                  :tag tag
                                  :scope scope
                                  :project-dir project-dir
                                  :limit limit)]
    {:kind guidance-kind
     :schema_version schema-version
     :instruction "Before implementation or after a mismatch, review relevant local experiences. After discovering a new reusable lesson, call record_experience or fighorse experience add."
     :ai_contract (guidance/ai-contract)
     :output_locations (guidance/output-location-guidance)
     :record_when ["A Figma-to-code mismatch is fixed."
                   "A platform-specific rule is learned."
                   "An export format or asset pipeline issue is discovered."
                   "A prompt/workflow step prevents a repeated error."
                   "A real app screenshot reveals overlap, clipping, wrong typography, or wrong system chrome handling."]
     :schema (schema)
     :summary summary}))

(defn guidance->markdown [data]
  (let [records (get-in data [:summary :records])]
    (str "# fighorse Learned Experience\n\n"
         (:instruction data) "\n\n"
         "Store: `" (get-in data [:summary :store_path]) "`\n\n"
         "## When To Record\n\n"
         (str/join "\n" (map #(str "- " %) (:record_when data)))
         "\n\n## Relevant Records\n\n"
         (if (seq records)
           (str/join "\n\n"
                     (map (fn [r]
                            (str "- **" (:summary r) "**"
                                 "\n  Lesson: " (:lesson r)
                                 (when-let [rec (:recommendation r)]
                                   (str "\n  Recommendation: " rec))))
                          records))
           "No matching local experience yet. Record lessons after this task."))))

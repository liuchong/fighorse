(ns fighorse.install
  "Installation artifact generation for fighorse runtime, MCP clients, services, and skills."
  (:require [clojure.string :as str]
            [fighorse.config :as config]
            [fighorse.experience :as experience]
            [fighorse.guidance :as guidance]))

(def ^:private fs (js/require "fs"))
(def ^:private path (js/require "path"))
(def ^:private os (js/require "os"))
(def ^:private child-process (js/require "child_process"))

(def supported-clients
  ["cursor" "codex" "kimi" "kimi-cli" "claude" "opencode" "openclaw" "hermes-agent" "generic"])

(defn- now-iso []
  (.toISOString (js/Date.)))

(defn- safe-timestamp []
  (str/replace (now-iso) #"[:.]" "-"))

(defn- join-path [& parts]
  (.apply (.-join path) path (to-array parts)))

(defn- homedir []
  (.homedir os))

(defn- file-exists? [file]
  (.existsSync fs file))

(defn- mkdirp! [dir]
  (.mkdirSync fs dir #js {:recursive true})
  dir)

(defn- write-text! [file content]
  (mkdirp! (.dirname path file))
  (.writeFileSync fs file content)
  file)

(defn- write-json! [file data]
  (write-text! file (js/JSON.stringify (clj->js data) nil 2)))

(defn- read-json-object [file]
  (if (file-exists? file)
    (try
      (let [data (js->clj (js/JSON.parse (.readFileSync fs file "utf8")))]
        (if (map? data) data {}))
      (catch :default _
        {}))
    {}))

(defn- backup-file! [file]
  (when (file-exists? file)
    (let [backup (str file ".bak." (safe-timestamp))]
      (.copyFileSync fs file backup)
      backup)))

(defn- write-text-with-backup! [file content]
  (let [current (when (file-exists? file) (.readFileSync fs file "utf8"))]
    (when (not= current content)
      (backup-file! file)
      (write-text! file content)))
  file)

(defn- write-json-with-backup! [file data]
  (write-text-with-backup! file (js/JSON.stringify (clj->js data) nil 2)))

(defn- mask-token [token]
  (when-not (str/blank? token)
    (str (subs token 0 (min 6 (count token))) "...")))

(defn- home-dir [& {:keys [home]}]
  (or home (config/fighorse-home)))

(defn- normalize-client [client]
  (let [client (str/lower-case (or client "generic"))]
    (case client
      "kimi-cli" "kimi"
      (if (some #(= client %) supported-clients) client "generic"))))

(defn- split-list [s]
  (if (str/blank? s)
    []
    (->> (str/split s #",")
         (map str/trim)
         (remove str/blank?)
         vec)))

(defn coerce-clients [client clients]
  (let [items (cond
                (sequential? clients) clients
                (string? clients) (split-list clients)
                (string? client) [client]
                :else ["generic"])]
    (->> items
         (map normalize-client)
         distinct
         vec)))

(defn- path-dirs []
  (->> (str/split (or (.-PATH js/process.env) "") (js/RegExp. (or (.-delimiter path) ":") "g"))
       (remove str/blank?)
       vec))

(defn- executable-candidates [command]
  (let [exts (if (= "win32" (.-platform js/process))
               ["" ".exe" ".cmd" ".bat"]
               [""])]
    (cond
      (str/blank? command) []
      (or (.isAbsolute path command)
          (str/includes? command "/")
          (str/includes? command "\\"))
      (mapv #(str command %) exts)
      :else
      (vec (for [dir (path-dirs)
                 ext exts]
             (join-path dir (str command ext)))))))

(defn executable-path [command]
  (some (fn [candidate]
          (when (file-exists? candidate)
            candidate))
        (executable-candidates command)))

(defn- run-command! [command args & {:keys [env ignore-error]}]
  (let [result (.spawnSync child-process command (clj->js (vec args))
                           #js {:encoding "utf8"
                                :stdio "pipe"
                                :env (js/Object.assign #js {} js/process.env (clj->js (or env {})))})
        error (.-error result)
        status (.-status result)
        exit-code (if (nil? status) (if error 1 0) status)
        data {:command command
              :args (vec args)
              :exit_code exit-code
              :ok (zero? exit-code)
              :stdout (or (.-stdout result) "")
              :stderr (or (.-stderr result) "")
              :error (when error (.-message error))}]
    (when (and (not ignore-error) (not (:ok data)))
      (throw (js/Error. (str "Command failed: " command " " (str/join " " args) "\n"
                             (or (:stderr data) (:error data) "")))))
    data))

(defn- mcp-stdio-config [command home]
  {:command command
   :args ["mcp" "serve" "--transport" "stdio"]
   :env {:FIGHORSE_MCP_MODE "readonly"
         :FIGHORSE_MCP_LOCAL_WRITE "allow"
         :FIGHORSE_HOME (home-dir :home home)}})

(defn- mcp-sse-config [port]
  {:transport "sse"
   :url (str "http://127.0.0.1:" port "/sse")})

(defn mcp-server-config
  [& {:keys [transport port command home]
      :or {transport "stdio" port 9449 command "fighorse"}}]
  (if (= "sse" transport)
    (mcp-sse-config port)
    (mcp-stdio-config command home)))

(defn- codex-toml [command home]
  (str "[mcp_servers.fighorse]\n"
       "command = \"" command "\"\n"
       "args = [\"mcp\", \"serve\", \"--transport\", \"stdio\"]\n"
       "enabled = true\n"
       "startup_timeout_sec = 60\n\n"
       "[mcp_servers.fighorse.env]\n"
       "FIGHORSE_MCP_MODE = \"readonly\"\n"
       "FIGHORSE_MCP_LOCAL_WRITE = \"allow\"\n"
       "FIGHORSE_HOME = \"" (home-dir :home home) "\"\n"))

(defn skill-markdown []
  (str "---\n"
       "name: fighorse\n"
       "description: Recreate, inspect, export, and debug Figma designs with fighorse CLI or MCP. Use when the user asks to replicate a Figma design, inspect Figma nodes/tokens, export Figma assets, or connect an AI coding tool to Figma.\n"
       "---\n\n"
       "# fighorse\n\n"
       "Use fighorse when a user asks to recreate, inspect, export, or debug a Figma design.\n\n"
       "## Discovery\n\n"
       "1. Call `fighorse discover --format json` or MCP `discover_fighorse` first.\n"
       "2. Call `fighorse experience summary --platform <platform> --asset-format <format>` or MCP `list_experiences` before implementation.\n"
       "3. If platform or asset format is missing, ask the developer before choosing.\n\n"
       "## Replication\n\n"
       "Use `get_design_package` or `fighorse design package <figma-url> --platform <platform> --asset-format <format>` as the main context source. "
       "Prioritize screenshots, learned_experience, explicit typography, tokens, compact tree metadata, then assets.\n\n"
       "## Assets\n\n"
       "Use `export_images`, `export_component`, or `download_image_fills` with `manifest=true` for local slices, controls, icons, and image fills. "
       "MCP export requires `FIGHORSE_MCP_LOCAL_WRITE=allow` and still only writes inside approved export roots. "
       "Use `./.fighorse/exports` for temporary slices, `./assets/fighorse` or the app resource directory for packaged assets, and `~/.fighorse/exports` for cross-project scratch data. "
       "Do not write exports to protected system paths, dependency caches, or hard-to-discover temporary locations unless explicitly requested.\n\n"
       "## Feedback Loop\n\n"
       "Build/run the target app, capture screenshots, compare with Figma, fix overlap/clipping/status-bar/typography issues, then call `record_experience` or `fighorse experience add` for reusable lessons.\n"))

(defn agents-markdown []
  (str "# fighorse Agent Instructions\n\n"
       "- Start with `fighorse discover --format json` or MCP `discover_fighorse`.\n"
       "- Load local lessons with `list_experiences` before using `get_design_package`.\n"
       "- Use `platform` and `asset_format` explicitly; ask if unknown.\n"
       "- Export assets with manifests instead of inventing controls or icons.\n"
       "- Store temporary exports in `./.fighorse/exports`; store packaged assets in `./assets/fighorse` or the app resource directory; MCP export requires `FIGHORSE_MCP_LOCAL_WRITE=allow` and path validation.\n"
       "- Record reusable fixes with `record_experience` after visual debugging.\n"))

(defn cursor-rule []
  (str "---\n"
       "description: Use fighorse for Figma design replication\n"
       "alwaysApply: false\n"
       "---\n\n"
       (agents-markdown)))

(defn- default-binary-target [& {:keys [home]}]
  (join-path (home-dir :home home) "bin" "fighorse"))

(defn- path-preferred-link-dirs []
  (let [home (homedir)
        candidates [(join-path home "bin")
                    (join-path home ".local" "bin")
                    "/usr/local/bin"]]
    (->> candidates
         (filter (fn [dir] (some #(= dir %) (path-dirs))))
         distinct
         vec)))

(defn- copy-executable! [source target]
  (when (str/blank? source)
    (throw (js/Error. "--source is required when applying binary installation")))
  (when-not (file-exists? source)
    (throw (js/Error. (str "Binary source not found: " source))))
  (mkdirp! (.dirname path target))
  (.copyFileSync fs source target)
  (try
    (.chmodSync fs target 493)
    (catch :default _ nil))
  target)

(defn- symlink-or-copy! [target link]
  (mkdirp! (.dirname path link))
  (when (file-exists? link)
    (.unlinkSync fs link))
  (try
    (.symlinkSync fs target link)
    (catch :default _
      (.copyFileSync fs target link)
      (try
        (.chmodSync fs link 493)
        (catch :default _ nil))))
  link)

(defn install-binary!
  [& {:keys [source target link-dir link-dirs home apply]
      :or {apply false}}]
  (let [apply (boolean apply)
        target (or target (default-binary-target :home home))
        link-dirs (vec (concat (split-list link-dirs)
                               (when link-dir [link-dir])
                               (path-preferred-link-dirs)))
        link-dirs (->> link-dirs (remove str/blank?) distinct vec)
        links (mapv #(join-path % "fighorse") link-dirs)
        applied (when apply
                  {:binary (copy-executable! source target)
                   :links (mapv #(symlink-or-copy! target %) links)})]
    {:kind "fighorse.install-binary.v1"
     :apply apply
     :source source
     :target target
     :links links
     :path_contains_target_dir (some #(= (.dirname path target) %) (path-dirs))
     :path_link_dirs link-dirs
     :applied applied
     :next_steps (if apply
                   ["Run `fighorse doctor` to verify the installed CLI."
                    "Use the absolute target path in GUI MCP clients when PATH inheritance is uncertain."]
                   [(str "Run `fighorse install binary --apply --source <binary> --target " target "` to install the CLI.")
                    "Use --link-dir or --link-dirs to add command links in PATH directories."])}))

(defn- write-skill-set! [base]
  (let [skill (join-path base "SKILL.md")
        agents (join-path base "AGENTS.md")
        cursor (join-path base "cursor-rule.mdc")]
    (write-text! skill (skill-markdown))
    (write-text! agents (agents-markdown))
    (write-text! cursor (cursor-rule))
    [skill agents cursor]))

(defn- client-skill-targets [client]
  (let [home (homedir)]
    (case (normalize-client client)
      "cursor" [{:kind "skill" :dir (join-path home ".cursor" "skills" "fighorse")}
                {:kind "rule" :file (join-path home ".cursor" "rules" "fighorse.mdc")}]
      "codex" [{:kind "skill" :dir (join-path home ".codex" "skills" "fighorse")}]
      "kimi" [{:kind "skill" :dir (join-path home ".kimi" "skills" "fighorse")}]
      "generic" [{:kind "skill" :dir (join-path home ".config" "agents" "skills" "fighorse")}
                 {:kind "skill" :dir (join-path home ".agents" "skills" "fighorse")}]
      [])))

(defn- apply-skill-target! [target]
  (case (:kind target)
    "rule" (do
             (write-text-with-backup! (:file target) (cursor-rule))
             {:kind "rule" :file (:file target)})
    (let [files (write-skill-set! (:dir target))]
      {:kind "skill" :dir (:dir target) :files files})))

(defn- apply-skills! [clients]
  (->> (concat [{:kind "skill" :dir (join-path (homedir) ".config" "agents" "skills" "fighorse")}]
               (mapcat client-skill-targets clients))
       distinct
       (mapv apply-skill-target!)))

(defn- migrate-legacy-cursor-internal-skill! []
  (let [home (homedir)
        legacy-dir (join-path home ".cursor" "skills-cursor" "fighorse")
        manifest-file (join-path home ".cursor" "skills-cursor" ".sync-manifest.json")
        backup-dir (join-path (config/fighorse-home)
                              "backups"
                              (str "cursor-skills-cursor-fighorse-" (safe-timestamp)))
        moved (when (file-exists? legacy-dir)
                (mkdirp! (.dirname path backup-dir))
                (.renameSync fs legacy-dir backup-dir)
                {:from legacy-dir :to backup-dir})
        manifest-updated (when (file-exists? manifest-file)
                           (let [manifest (read-json-object manifest-file)]
                             (if (get-in manifest ["skills" "fighorse"])
                               (let [updated (update manifest "skills" dissoc "fighorse")]
                                 (write-json-with-backup! manifest-file updated)
                                 manifest-file)
                               nil)))]
    (when (or moved manifest-updated)
      {:legacy_dir moved
       :manifest manifest-updated
       :reason "Cursor documents ~/.cursor/skills-cursor as an internal system-managed directory; personal skills belong in ~/.cursor/skills."})))

(defn- migrate-legacy-config! [home]
  (let [legacy (config/legacy-config-path)
        target (join-path home "config.json")]
    (when (and (file-exists? legacy) (not (file-exists? target)))
      (mkdirp! (.dirname path target))
      (.copyFileSync fs legacy target)
      (try
        (.chmodSync fs target 384)
        (catch :default _ nil))
      {:from legacy :to target})))

(defn install-auth!
  [& {:keys [token home apply]
      :or {apply false}}]
  (let [apply (boolean apply)
        home (home-dir :home home)
        config-file (join-path home "config.json")
        current-token (or (:token (read-json-object config-file))
                          (:token (config/load-config)))
        token (some-> token str/trim)]
    (cond
      (not apply)
      {:kind "fighorse.install-auth.v1"
       :apply false
       :config_path config-file
       :has_saved_token (not (str/blank? current-token))
       :token_mask (mask-token current-token)
       :next_steps ["Run `fighorse install auth --apply --token <FIGMA_TOKEN>` to persist a Figma token."
                    "You can also pipe the token on stdin to avoid exposing it in shell history."
                    "MCP clients inherit this saved config through FIGHORSE_HOME."]}

      (str/blank? token)
      (if (str/blank? current-token)
        {:kind "fighorse.install-auth.v1"
         :apply true
         :ok false
         :config_path config-file
         :has_saved_token false
         :needs_token true
         :next_steps ["Provide a Figma token with `--token`, pipe it on stdin, or run `fighorse auth login`."
                      "Do not commit tokens; fighorse stores them only in the local user config."]}
        (do
          (mkdirp! home)
          (write-json-with-backup! config-file {:token current-token})
          (try
            (.chmodSync fs config-file 384)
            (catch :default _ nil))
          {:kind "fighorse.install-auth.v1"
           :apply true
           :ok true
           :config_path config-file
           :has_saved_token true
           :token_mask (mask-token current-token)
           :next_steps ["Existing saved Figma token was kept."
                        "Run `fighorse doctor` or `fighorse smoke <figma-url>` to verify Figma access."]}))

      :else
      (do
        (mkdirp! home)
        (write-json-with-backup! config-file {:token token})
        (try
          (.chmodSync fs config-file 384)
          (catch :default _ nil))
        {:kind "fighorse.install-auth.v1"
         :apply true
         :ok true
         :config_path config-file
         :has_saved_token true
         :token_mask (mask-token token)
         :next_steps ["Run `fighorse doctor` or `fighorse smoke <figma-url>` to verify Figma access."]}))))

(defn install-home!
  [& {:keys [home]}]
  (let [home (home-dir :home home)
        dirs [(mkdirp! home)
              (mkdirp! (.join path home "bin"))
              (mkdirp! (.join path home "experience"))
              (mkdirp! (.join path home "clients"))
              (mkdirp! (.join path home "services"))
              (mkdirp! (.join path home "skills"))
              (mkdirp! (.join path home "logs"))
              (mkdirp! (.join path home "runtime"))
              (mkdirp! (.join path home "exports"))]
        readme (.join path home "README.md")]
    (write-text! readme
                 (str "# fighorse Home\n\n"
                      "This directory stores local fighorse configuration, global experience, generated MCP client snippets, service files, skills, logs, runtime files, and exported assets.\n\n"
                      "- Global experience: `experience/global.jsonl`\n"
                      "- Project experience: `<project>/.fighorse/experience.jsonl` after `fighorse install project`\n"
                      "- Override home with `FIGHORSE_HOME`.\n"))
    {:kind "fighorse.install-home.v1"
     :home home
     :created_at (now-iso)
     :directories dirs
     :files [readme]
     :migrated_config (migrate-legacy-config! home)
     :ai_contract (guidance/ai-contract)
     :experience (experience/store-info)}))

(defn install-project!
  [& {:keys [project-dir]}]
  (let [project-dir (or project-dir (.cwd js/process))
        dir (.join path project-dir ".fighorse")
        config-file (.join path dir "fighorse.json")
        ignore-file (.join path dir ".gitignore")
        readme-file (.join path dir "README.md")
        config {:kind "fighorse.project.v1"
                :schema_version 1
                :created_at (now-iso)
                :experience {:scope "project"
                             :write_path ".fighorse/experience.jsonl"
                             :read_order ["project" "global"]
                             :compatibility "Experience records are append-only JSONL; readers ignore unknown fields."}
                :exports {:scratch ".fighorse/exports"
                          :packaged "assets/fighorse"
                          :manifest_required true}
                :ai {:default_workflow ["discover_fighorse" "list_experiences" "get_design_package" "record_experience"]
                     :must_obey (:must (guidance/ai-contract))
                     :ask_when_missing ["platform" "asset_format"]}}]
    (mkdirp! dir)
    (write-json! config-file config)
    (write-text! ignore-file "experience*.jsonl\nexports/\nlogs/\nruntime/\n")
    (write-text! readme-file
                 (str "# fighorse Project\n\n"
                      "This project is initialized for project-scoped fighorse experience.\n\n"
                      "- Write path: `.fighorse/experience.jsonl`\n"
                      "- Temporary exports: `.fighorse/exports`\n"
                      "- Packaged assets: `assets/fighorse` or the app's normal resource directory\n"
                      "- Reads merge project experience first and global experience second.\n"
                      "- Keep `fighorse.json` in source control if the team wants consistent AI behavior.\n"))
    {:kind "fighorse.install-project.v1"
     :project_dir project-dir
     :files [config-file ignore-file readme-file]
     :ai_contract (guidance/ai-contract)
     :experience (experience/store-info :scope "project" :project-dir project-dir)}))

(defn install-skill!
  [& {:keys [dir home apply clients client]
      :or {apply false}}]
  (let [apply (boolean apply)
        base (or dir (.join path (home-dir :home home) "skills" "fighorse"))
        files (write-skill-set! base)
        selected-clients (coerce-clients client clients)
        applied (when apply (apply-skills! selected-clients))]
    {:kind "fighorse.install-skill.v1"
     :dir base
     :files files
     :apply apply
     :clients selected-clients
     :applied applied
     :ai_contract (guidance/ai-contract)
     :usage ["Attach SKILL.md as a skill where supported."
             "Copy AGENTS.md into an AI coding project when a generic agent instruction file is preferred."
             "Copy cursor-rule.mdc into .cursor/rules/fighorse.mdc for Cursor project rules."
             "Use `--apply --clients cursor,codex,kimi` to install known user-level skills/rules."]}))

(defn- cursor-mcp-payload [server]
  (-> server
      (assoc :name "fighorse")
      (dissoc :transport :type)))

(defn- merge-json-mcp-config! [file server]
  (let [current (read-json-object file)
        updated (assoc-in current ["mcpServers" "fighorse"] server)]
    (write-json-with-backup! file updated)
    {:method "json-config"
     :ok true
     :file file}))

(defn- codex-managed-block [command home]
  (str "# BEGIN fighorse managed\n"
       (codex-toml command home)
       "# END fighorse managed\n"))

(defn- ensure-codex-startup-timeout-text [content]
  (let [section-re #"(\[mcp_servers\.fighorse\]\n)([\s\S]*?)(?=\n\[|$)"]
    (if-not (re-find section-re content)
      content
      (str/replace content section-re
                   (fn [[_ header body]]
                     (let [body (if (re-find #"(?m)^startup_timeout_sec\s*=" body)
                                  (str/replace body #"(?m)^startup_timeout_sec\s*=.*$" "startup_timeout_sec = 60")
                                  (str body "startup_timeout_sec = 60\n"))]
                       (str header body)))))))

(defn- ensure-codex-startup-timeout! [file]
  (let [current (if (file-exists? file) (.readFileSync fs file "utf8") "")
        updated (ensure-codex-startup-timeout-text current)]
    (cond
      (str/blank? current)
      {:method "codex-startup-timeout"
       :ok false
       :file file
       :skipped true
       :reason "Codex config file does not exist yet."}

      (= current updated)
      {:method "codex-startup-timeout"
       :ok true
       :file file
       :skipped true
       :reason "startup_timeout_sec is already present or no fighorse server block was found."}

      :else
      (do
        (write-text-with-backup! file updated)
        {:method "codex-startup-timeout"
         :ok true
         :file file
         :startup_timeout_sec 60}))))

(defn- merge-codex-config! [file command home]
  (let [block (codex-managed-block command home)
        current (if (file-exists? file) (.readFileSync fs file "utf8") "")
        updated (cond
                  (str/includes? current "# BEGIN fighorse managed")
                  (str/replace current #"# BEGIN fighorse managed[\s\S]*?# END fighorse managed\n?" block)

                  (str/includes? current "[mcp_servers.fighorse]")
                  current

                  :else
                  (str current (when-not (str/blank? current) "\n\n") block))]
    (if (= current updated)
      {:method "toml-config"
       :ok false
       :file file
       :skipped true
       :reason "Existing unmarked [mcp_servers.fighorse] block found; not overwriting user-managed config."}
      (do
        (write-text-with-backup! file updated)
        {:method "toml-config" :ok true :file file}))))

(defn- apply-cursor-client! [server]
  (let [cursor (executable-path "cursor")
        json (js/JSON.stringify (clj->js (cursor-mcp-payload server)))
        agent-config-file (join-path (homedir) ".cursor" "mcp.json")]
    (if cursor
      (let [result (run-command! cursor ["--add-mcp" json] :ignore-error true)]
        (if (:ok result)
          {:method "cursor-cli"
           :ok true
           :result result
           :agent_config (merge-json-mcp-config! agent-config-file server)
           :agent_enable (run-command! cursor ["agent" "mcp" "enable" "fighorse"] :ignore-error true)}
          {:method "cursor-cli-with-json-fallback"
           :ok true
           :result result
           :fallback (merge-json-mcp-config! agent-config-file server)
           :agent_enable (run-command! cursor ["agent" "mcp" "enable" "fighorse"] :ignore-error true)}))
      (merge-json-mcp-config! agent-config-file server))))

(defn- apply-codex-client! [server command home]
  (let [codex (executable-path "codex")
        config-file (join-path (homedir) ".codex" "config.toml")]
    (if codex
      (let [remove-result (run-command! codex ["mcp" "remove" "fighorse"] :ignore-error true)
            add-result (run-command! codex (concat ["mcp" "add"
                                                    "--env" (str "FIGHORSE_HOME=" (home-dir :home home))
                                                    "--env" "FIGHORSE_MCP_MODE=readonly"
                                                   "--env" "FIGHORSE_MCP_LOCAL_WRITE=allow"
                                                    "fighorse" "--"]
                                                   [command "mcp" "serve" "--transport" "stdio"])
                                     :ignore-error true)]
        (if (:ok add-result)
          {:method "codex-cli"
           :ok true
           :remove remove-result
           :result add-result
           :config_patch (ensure-codex-startup-timeout! config-file)}
          (let [fallback (merge-codex-config! config-file command home)]
            {:method "codex-cli-with-toml-fallback"
             :ok true
             :remove remove-result
             :result add-result
             :fallback fallback
             :config_patch (ensure-codex-startup-timeout! config-file)})))
      (let [fallback (merge-codex-config! config-file command home)]
        (assoc fallback :config_patch (ensure-codex-startup-timeout! config-file))))))

(defn- apply-kimi-client! [server command home]
  (let [kimi (or (executable-path "kimi") (executable-path "kimi-cli"))]
    (if kimi
      (let [result (run-command! kimi (concat ["mcp" "add"
                                               "--transport" "stdio"
                                               "--env" (str "FIGHORSE_HOME=" (home-dir :home home))
                                               "--env" "FIGHORSE_MCP_MODE=readonly"
                                               "--env" "FIGHORSE_MCP_LOCAL_WRITE=allow"
                                               "fighorse" "--"]
                                              [command "mcp" "serve" "--transport" "stdio"])
                                :ignore-error true)]
        (if (:ok result)
          {:method "kimi-cli"
           :ok true
           :result result}
          {:method "kimi-cli-with-json-fallback"
           :ok true
           :result result
           :fallback (merge-json-mcp-config! (join-path (homedir) ".kimi" "mcp.json") server)}))
      (merge-json-mcp-config! (join-path (homedir) ".kimi" "mcp.json") server))))

(defn- apply-client! [client server command home]
  (let [client (normalize-client client)]
    (if-not (#{"cursor" "codex" "kimi" "generic"} client)
      {:client client
       :ok false
       :skipped true
       :reason "No verified apply strategy for this client yet; generated reviewable artifacts only."}
      (let [config-result (case client
                            "cursor" (apply-cursor-client! server)
                            "codex" (apply-codex-client! server command home)
                            "kimi" (apply-kimi-client! server command home)
                            "generic" (merge-json-mcp-config! (join-path (homedir) ".config" "agents" "mcp.json") server))
            skill-result (apply-skills! [client])]
        {:client client
         :ok (:ok config-result)
         :mcp config-result
         :skills skill-result
         :migration (when (= "cursor" client)
                      (migrate-legacy-cursor-internal-skill!))}))))

(defn client-detection [client]
  (let [client (normalize-client client)]
    (case client
      "cursor" {:client client
                :command (executable-path "cursor")
                :mcp_config (join-path (homedir) ".cursor" "mcp.json")
                :skill_dir (join-path (homedir) ".cursor" "skills" "fighorse")
                :rule_file (join-path (homedir) ".cursor" "rules" "fighorse.mdc")
                :apply_supported true
                :apply_methods ["cursor --add-mcp"
                                "~/.cursor/mcp.json for Cursor Agent"
                                "cursor agent mcp enable fighorse"
                                "json-config-fallback"]
                :skill_source "Cursor create-skill documents personal skills at ~/.cursor/skills/<skill-name>; ~/.cursor/skills-cursor is internal."}
      "codex" {:client client
               :command (executable-path "codex")
               :mcp_config (join-path (homedir) ".codex" "config.toml")
               :skill_dir (join-path (homedir) ".codex" "skills" "fighorse")
               :apply_supported true
               :apply_methods ["codex mcp add" "toml-managed-block-fallback"]}
      "kimi" {:client client
              :command (or (executable-path "kimi") (executable-path "kimi-cli"))
              :mcp_config (join-path (homedir) ".kimi" "mcp.json")
              :skill_dir (join-path (homedir) ".kimi" "skills" "fighorse")
              :apply_supported true
              :apply_methods ["kimi mcp add" "json-config-fallback"]}
      "generic" {:client client
                 :mcp_config (join-path (homedir) ".config" "agents" "mcp.json")
                 :skill_dir (join-path (homedir) ".config" "agents" "skills" "fighorse")
                 :apply_supported true
                 :apply_methods ["json-config"]}
      {:client client
       :artifact_generation_supported true
       :apply_supported false
       :reason "No verified user-level apply strategy for this client yet; generated snippets can still be installed manually or by a client-specific command."})))

(defn install-client!
  [& {:keys [client dir transport port command home apply]
      :or {client "generic" transport "stdio" port 9449 command "fighorse" apply false}}]
  (let [apply (boolean apply)
        client (normalize-client client)
        base (or dir (.join path (home-dir :home home) "clients" client))
        server (mcp-server-config :transport transport :port port :command command :home home)
        mcp-json (.join path base "mcp.json")
        manifest (.join path base "fighorse-client.json")
        readme (.join path base "README.md")
        base-files [(write-json! mcp-json {:mcpServers {:fighorse server}})
                    (write-json! manifest {:kind "fighorse.client-install.v1"
                                           :client client
                                           :transport transport
                                           :generated_at (now-iso)
                                           :mcp_server server
                                           :detected (client-detection client)
                                           :recommended_tool_order ["discover_fighorse" "check_fighorse_ready" "list_experiences" "get_design_package" "record_experience"]
                                           :ai_contract (guidance/ai-contract)
                                           :notes ["By default this command writes reviewable snippets only."
                                                   "Use --apply to install into detected user-level client config and skill/rule locations."
                                                   "For Codex, apply prefers `codex mcp add` and falls back to a managed TOML block."
                                                   "For Cursor, apply uses `cursor --add-mcp`, writes ~/.cursor/mcp.json for Cursor Agent CLI, and attempts `cursor agent mcp enable fighorse`."
                                                   "For Kimi, apply prefers `kimi mcp add` and falls back to ~/.kimi/mcp.json."]})
                    (write-text! readme
                                 (str "# fighorse " client " install\n\n"
                                      "Main MCP config: `mcp.json`.\n\n"
                                      "Run with `--apply` to install into detected client config and skill locations.\n\n"
                                      "Recommended order: discover_fighorse, check_fighorse_ready, list_experiences, get_design_package, record_experience.\n"))]
        extra-files (cond-> []
                      (= "codex" client)
                      (conj (write-text! (.join path base "codex-config.toml") (codex-toml command home)))
                      (= "cursor" client)
                      (conj (write-text! (.join path base "fighorse.cursor.mdc") (cursor-rule)))
                      (= "kimi" client)
                      (conj (write-text! (.join path base "KIMI.md") (agents-markdown)))
                      (#{"claude" "opencode" "openclaw" "hermes-agent" "generic"} client)
                      (conj (write-text! (.join path base "AGENTS.md") (agents-markdown))))
        files (vec (concat base-files extra-files))
        applied (when apply (apply-client! client server command home))]
    {:kind "fighorse.install-client.v1"
     :client client
     :dir base
     :transport transport
     :files files
     :apply apply
     :detected (client-detection client)
     :applied applied
     :ai_contract (guidance/ai-contract)
     :next_steps (if apply
                   ["Restart or reload the target AI client if it was already running."
                    "Run the client's MCP list command when available and verify fighorse appears."
                    "Ask the AI client to call discover_fighorse before Figma replication."]
                   ["Review generated files."
                    "Run with --apply to install into detected client config and skill locations."
                    "Run the client and verify fighorse tools appear."])}))

(defn- launchd-plist [command port home]
  (str "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n"
       "<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n"
       "<plist version=\"1.0\">\n"
       "<dict>\n"
       "  <key>Label</key><string>com.groupultra.fighorse.mcp</string>\n"
       "  <key>ProgramArguments</key>\n"
       "  <array><string>" command "</string><string>mcp</string><string>serve</string><string>--transport</string><string>sse</string><string>--host</string><string>127.0.0.1</string><string>--port</string><string>" port "</string></array>\n"
       "  <key>EnvironmentVariables</key>\n"
       "  <dict><key>FIGHORSE_HOME</key><string>" home "</string><key>FIGHORSE_MCP_MODE</key><string>readonly</string><key>FIGHORSE_MCP_LOCAL_WRITE</key><string>allow</string></dict>\n"
       "  <key>RunAtLoad</key><true/>\n"
       "  <key>KeepAlive</key><true/>\n"
       "  <key>StandardOutPath</key><string>" home "/logs/mcp.out.log</string>\n"
       "  <key>StandardErrorPath</key><string>" home "/logs/mcp.err.log</string>\n"
       "</dict>\n"
       "</plist>\n"))

(defn- systemd-unit [command port home]
  (str "[Unit]\nDescription=fighorse MCP service\n\n"
       "[Service]\n"
       "Environment=FIGHORSE_HOME=" home "\n"
       "Environment=FIGHORSE_MCP_MODE=readonly\n"
       "Environment=FIGHORSE_MCP_LOCAL_WRITE=allow\n"
       "ExecStart=" command " mcp serve --transport sse --host 127.0.0.1 --port " port "\n"
       "Restart=always\n"
       "WorkingDirectory=" home "\n\n"
       "[Install]\nWantedBy=default.target\n"))

(defn- apply-launchd-service! [file]
  (let [target (join-path (homedir) "Library" "LaunchAgents" "com.groupultra.fighorse.mcp.plist")
        label "gui"
        uid (.trim (or (:stdout (run-command! "id" ["-u"] :ignore-error true)) ""))
        domain (if (str/blank? uid) label (str label "/" uid))]
    (mkdirp! (.dirname path target))
    (write-text-with-backup! target (.readFileSync fs file "utf8"))
    {:file target
     :bootout (run-command! "launchctl" ["bootout" domain target] :ignore-error true)
     :bootstrap (run-command! "launchctl" ["bootstrap" domain target] :ignore-error true)
     :kickstart (run-command! "launchctl" ["kickstart" "-k" (str domain "/com.groupultra.fighorse.mcp")] :ignore-error true)}))

(defn- apply-systemd-service! [file]
  (let [target (join-path (homedir) ".config" "systemd" "user" "fighorse-mcp.service")]
    (mkdirp! (.dirname path target))
    (write-text-with-backup! target (.readFileSync fs file "utf8"))
    {:file target
     :daemon_reload (run-command! "systemctl" ["--user" "daemon-reload"] :ignore-error true)
     :enable_now (run-command! "systemctl" ["--user" "enable" "--now" "fighorse-mcp.service"] :ignore-error true)}))

(defn- apply-service! [service file]
  (case service
    "launchd" (apply-launchd-service! file)
    "systemd" (apply-systemd-service! file)
    {:skipped true
     :reason (str "No apply strategy for service manager: " service)}))

(defn install-service!
  [& {:keys [service port command home apply]
      :or {service "auto" port 9449 command "fighorse" apply false}}]
  (let [apply (boolean apply)
        home (home-dir :home home)
        service (if (= "auto" service)
                  (if (= "darwin" js/process.platform) "launchd" "systemd")
                  service)
        dir (.join path home "services")
        port (str port)
        file (case service
               "launchd" (.join path dir "com.groupultra.fighorse.mcp.plist")
               "systemd" (.join path dir "fighorse-mcp.service")
               (.join path dir "fighorse-mcp.service"))]
    (mkdirp! dir)
    (write-text! file
                 (if (= "launchd" service)
                   (launchd-plist command port home)
                   (systemd-unit command port home)))
    {:kind "fighorse.install-service.v1"
     :service service
     :transport "sse"
     :port (js/parseInt port)
     :file file
     :apply apply
     :applied (when apply (apply-service! service file))
     :next_steps (if (= "launchd" service)
                   [(str "launchctl bootstrap gui/$(id -u) " file)
                    "launchctl kickstart -k gui/$(id -u)/com.groupultra.fighorse.mcp"]
                   [(str "mkdir -p ~/.config/systemd/user && cp " file " ~/.config/systemd/user/")
                    "systemctl --user daemon-reload"
                    "systemctl --user enable --now fighorse-mcp.service"])}))

(defn install-all!
  [& {:keys [client clients transport port command home project-dir source target link-dir link-dirs apply service token]
      :or {client "generic" transport "stdio" port 9449 command "fighorse" apply false service "auto"}}]
  (let [apply (boolean apply)
        home (home-dir :home home)
        selected-clients (coerce-clients client clients)
        binary-target (or target (default-binary-target :home home))
        command (if (and apply source) binary-target command)]
    {:kind "fighorse.install-all.v1"
     :apply apply
     :clients selected-clients
     :home (install-home! :home home)
     :auth (install-auth! :home home
                          :token token
                          :apply apply)
     :binary (install-binary! :source source
                              :target binary-target
                              :link-dir link-dir
                              :link-dirs link-dirs
                              :home home
                              :apply (and apply (not (str/blank? source))))
     :project (install-project! :project-dir project-dir)
     :skill (install-skill! :home home
                            :clients selected-clients
                            :apply apply)
     :clients_result (mapv #(install-client! :client %
                                             :transport transport
                                             :port port
                                             :command command
                                             :home home
                                             :apply apply)
                           selected-clients)
     :service (install-service! :service service
                                :port port
                                :command command
                                :home home
                                :apply apply)}))

(defn status []
  (let [home (config/fighorse-home)
        normalized-clients (->> supported-clients (map normalize-client) distinct vec)]
    {:kind "fighorse.install-status.v1"
     :home home
     :home_exists (.existsSync fs home)
     :platform (.-platform js/process)
     :binary {:default_target (default-binary-target :home home)
              :current_fighorse (executable-path "fighorse")
              :path_dirs (path-dirs)
              :preferred_link_dirs (path-preferred-link-dirs)}
     :experience (experience/store-info)
     :clients_dir (.join path home "clients")
     :services_dir (.join path home "services")
     :skills_dir (.join path home "skills")
     :supported_clients supported-clients
     :detected_clients (into {}
                             (map (fn [client] [client (client-detection client)]))
                             normalized-clients)
     :apply_note "Commands are dry-run/artifact-generating by default; pass --apply to mutate detected client configs, skill locations, and service managers."}))

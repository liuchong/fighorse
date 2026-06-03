(ns fighorse.discovery
  "Self-description payloads for AI tools and MCP clients."
  (:require [clojure.string :as str]
            [fighorse.api.coverage :as api-coverage]
            [fighorse.config :as config]
            [fighorse.experience :as experience]
            [fighorse.guidance :as guidance]
            [fighorse.utils.url :as figma-url]))

(def version "0.1.0")
(def ^:private fs (js/require "fs"))
(def ^:private path (js/require "path"))

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
             (.join path dir (str command ext)))))))

(defn- executable-path [command]
  (some (fn [candidate]
          (when (.existsSync fs candidate)
            candidate))
        (executable-candidates command)))

(defn- home-exists? []
  (.existsSync fs (config/fighorse-home)))

(defn- mcp-lock-file []
  (.join path (config/fighorse-home) "runtime" "mcp.lock"))

(defn- read-json-object [file]
  (try
    (when (.existsSync fs file)
      (js->clj (js/JSON.parse (.readFileSync fs file "utf8")) :keywordize-keys true))
    (catch :default _
      nil)))

(defn- active-pid? [pid]
  (and (number? pid)
       (pos? pid)
       (try
         (.kill js/process pid 0)
         true
         (catch :default err
           (not= "ESRCH" (.-code err))))))

(defn mcp-service-status []
  (let [lock-file (mcp-lock-file)
        lock (read-json-object lock-file)
        pid (:pid lock)]
    {:endpoint "http://127.0.0.1:9449/mcp"
     :health "http://127.0.0.1:9449/health"
     :lock_file lock-file
     :lock_present (boolean lock)
     :pid pid
     :running (active-pid? pid)
     :next_step (if (active-pid? pid)
                  "Ask the client to call discover_fighorse, or run fighorse doctor --format json."
                  "For MCP clients, run fighorse install --default --mode service --clients cursor,codex,kimi --apply.")}))

(defn setup-guidance
  "Structured setup instructions for humans and AI clients."
  []
  {:kind "fighorse.setup-guidance.v1"
   :required_for_figma_api
   [{:id "figma_token"
     :name "Figma Personal Access Token"
     :why "fighorse uses the public Figma REST API; file, node, image, component, variable, comment, webhook, analytics, and design-package calls require a token."
     :accepted_sources ["FIGMA_TOKEN environment variable"
                        "FIGMA_API_KEY environment variable"
                        "~/.fighorse/config.json written by fighorse auth login"]
     :recommended_command "fighorse auth login --token <FIGMA_TOKEN>"
     :one_shot_command "FIGMA_TOKEN=<FIGMA_TOKEN> fighorse quickstart \"<figma-frame-url>\""
     :safety "Do not paste tokens into prompts, commits, screenshots, or shared logs. Store them through local config or environment variables only."}]
   :recommended_first_run
   ["Run fighorse auth login --token <FIGMA_TOKEN>."
    "Copy a link to a selected Figma frame, component, or group so the URL includes node-id."
    "Run fighorse quickstart \"<figma-frame-url>\"."
    "Run fighorse design package \"<figma-frame-url>\" --platform <target-platform> --asset-format <asset-format>."]
   :optional_mcp_service
   {:when "Only when an AI client such as Cursor, Codex, or Kimi should call fighorse directly."
    :command "fighorse install --default --mode service --clients cursor,codex,kimi --apply"
    :endpoint "http://127.0.0.1:9449/mcp"}
   :ai_client_behavior
   {:must_check_first ["discover_fighorse" "check_fighorse_ready"]
    :if_auth_missing "Do not call Figma API tools yet. Tell the user: fighorse needs a Figma Personal Access Token. Run `fighorse auth login --token <FIGMA_TOKEN>` or set FIGMA_TOKEN, then retry."
    :if_url_missing "Ask the user to paste a specific Figma frame/group/component link, not a full canvas link."
    :if_platform_or_asset_format_missing "Ask the user for target platform/framework and preferred asset format before implementation."
    :after_user_fixes_setup "Call check_fighorse_ready again, then parse_figma_url and get_design_package."}})

(defn quickstart
  "Read-only guided readiness report for new users."
  [& {:keys [figma-url]}]
  (let [{:keys [token config-path proxy]} (config/load-config)
        parsed (when-not (str/blank? figma-url)
                 (figma-url/parse-figma-url figma-url))
        has-token? (boolean (seq token))
        has-url? (boolean parsed)
        exact-selection? (boolean (:node_id parsed))
        binary (or (executable-path "fighorse")
                   (when (.existsSync fs "./dist/fighorse")
                     (.resolve path "./dist/fighorse")))
        checks [{:id "runtime"
                 :ok (boolean (some-> js/globalThis .-Bun .-version))
                 :message (if (some-> js/globalThis .-Bun .-version)
                            "Bun runtime is available."
                            "Bun runtime is not available. Run fighorse with Bun or the compiled binary.")}
                {:id "binary"
                 :ok (boolean binary)
                 :message (if binary
                            (str "fighorse binary found at " binary)
                            "Build and install fighorse before using it globally.")
                 :next_command "From source: bun run install:local. From a downloaded binary: ./fighorse install --default --apply."}
                {:id "auth"
                 :ok has-token?
                 :message (if has-token?
                            "Figma token is configured."
                            "Figma token is missing.")
                 :next_command "fighorse auth login --token <FIGMA_TOKEN>"}
                {:id "figma_url"
                 :ok (and has-url? (:valid parsed))
                 :message (cond
                            (not has-url?) "Paste a Figma frame, component, or group link to continue."
                            (:valid parsed) "Figma URL or file key parsed successfully."
                            :else (:error parsed))
                 :next_command "fighorse quickstart \"<figma-frame-url>\""}
                {:id "specific_frame"
                 :ok exact-selection?
                 :message (cond
                            exact-selection? "The link includes node-id, so it targets a specific selection."
                            has-url? "The link has no node-id. Copy a link to a selected frame, component, or group for best results."
                            :else "Copy a link to a selected frame, component, or group.")
                 :next_command "In Figma: right click the frame or group, then copy link to selection."}]
        ready? (every? :ok checks)
        design-command (when (and (:valid parsed) (:file_key parsed))
                         (str "fighorse design package \"" (or figma-url (:file_key parsed))
                              "\" --platform <target-platform> --asset-format <asset-format> --output ./.fighorse/exports/package.json"))]
    {:kind "fighorse.quickstart.v1"
     :status (if ready? "ready" "needs-action")
     :summary (if ready?
                "Ready to build a design package."
                "Follow next_steps before building a design package.")
     :checks checks
     :auth {:has_token has-token?
            :config_path config-path}
     :install {:home (config/fighorse-home)
               :home_exists (home-exists?)
               :binary binary
               :default_mode "cli"
               :service_mode "explicit: fighorse install --default --mode service --clients cursor,codex,kimi --apply"}
     :mcp (mcp-service-status)
     :setup (setup-guidance)
     :figma_url parsed
     :proxy {:configured (boolean proxy)
             :value proxy}
     :next_steps (cond-> []
                   (not binary) (conj "Build and install from source: bun run install:local. Or install a downloaded binary: ./fighorse install --default --apply.")
                   (not has-token?) (conj "Add a Figma token: fighorse auth login --token <FIGMA_TOKEN>")
                   (not has-url?) (conj "Copy a link to a specific Figma frame, component, or group.")
                   (and has-url? (not exact-selection?)) (conj "Narrow the input to an exact Figma selection with node-id.")
                   design-command (conj design-command)
                   :always (conj "Optional MCP service: fighorse install --default --mode service --clients cursor,codex,kimi --apply"))}))

(defn quickstart->markdown [report]
  (let [line (fn [{:keys [ok id message next_command]}]
               (str "- " (if ok "OK" "TODO") " `" id "`: " message
                    (when (and (not ok) next_command)
                      (str "\n  Next: `" next_command "`"))))]
    (str "# fighorse Quickstart\n\n"
         (:summary report) "\n\n"
         "## Required Figma Setup\n\n"
         "1. Save a Figma Personal Access Token before calling Figma APIs:\n"
         "   `fighorse auth login --token <FIGMA_TOKEN>`\n"
         "   Or run one command with `FIGMA_TOKEN=<token> fighorse ...`.\n"
         "2. Copy a specific Figma frame, group, or component link. Avoid whole-canvas links for implementation.\n"
         "3. Run quickstart again with the selected link:\n"
         "   `fighorse quickstart \"<figma-frame-url>\"`\n"
         "4. Build an AI-ready design package:\n"
         "   `fighorse design package \"<figma-frame-url>\" --platform <target> --asset-format <format>`\n"
         "5. Optional MCP service for Cursor/Codex/Kimi:\n"
         "   `fighorse install --default --mode service --clients cursor,codex,kimi --apply`\n\n"
         "## AI Client Setup Rule\n\n"
         "First run `fighorse quickstart --format json` or MCP `check_fighorse_ready`. "
         "If `auth.has_token=false`, do not call Figma API tools yet. "
         "Tell the user: fighorse needs a Figma Personal Access Token; run "
         "`fighorse auth login --token <FIGMA_TOKEN>` or set `FIGMA_TOKEN`, then retry.\n\n"
         "## Checks\n\n"
         (str/join "\n" (map line (:checks report)))
         "\n\n## Next Steps\n\n"
         (str/join "\n" (map #(str "- " %) (:next_steps report)))
         "\n")))

(defn manifest []
  {:kind "fighorse.discovery.v1"
   :name "fighorse"
   :version version
   :purpose "Provide public-first Figma CLI + MCP infrastructure for design context, screenshots, tokens, assets, diagnostics, and implementation hints."
   :primary_use_case "Given a specific Figma frame URL, produce enough structured context for an AI coding tool to recreate the selected design."
   :production_defaults
   {:mcp_mode "readonly"
    :mcp_local_write "set FIGHORSE_MCP_LOCAL_WRITE=allow only for safe local asset exports"
    :fighorse_home "~/.fighorse"
    :global_experience "~/.fighorse/experience/global.jsonl"
    :project_experience "./.fighorse/experience.jsonl after fighorse install project"
    :quickstart "fighorse quickstart \"<figma-frame-url>\""
    :auth_setup "fighorse auth login --token <FIGMA_TOKEN>"
    :default_design_package {:depth 2
                             :max_tokens 8000
                             :include_screenshot true
                             :include_assets false
                             :platform "ask-developer-if-unspecified"
                             :asset_format "ask-developer-if-unspecified; png is only the render fallback"}
    :smoke_test "fighorse smoke <figma-url>"}
   :setup_requirements (setup-guidance)
   :input_contract
   {:preferred "figma_url"
    :accepted ["Figma design/file/proto/board URL"
               "raw Figma file_key plus optional node_id"
               "optional target platform/framework, e.g. android-compose"
               "optional preferred asset format, e.g. png or svg"]
    :node_id_note "Figma URLs use node-id=1-2. Figma REST APIs use 1:2. fighorse normalizes this automatically."}
   :output_contracts
   {:design_package
    {:kind "fighorse.design-package.v1"
     :contains ["source" "file" "target" "implementation_target" "screen_candidates" "component_candidates" "fidelity_workflow" "asset_export_plan" "learned_experience" "context" "tokens" "token_confidence" "missing_font_diagnostics" "screenshots" "assets" "implementation_risk_checklist" "implementation_hints"]
     :best_for "AI design replication and implementation planning"}
    :experience_record
    {:kind experience/record-kind
     :schema_version experience/schema-version
     :best_for "Persisting reusable lessons from real Figma replication, screenshot comparison, asset export, and platform debugging."}}
   :api_coverage (api-coverage/coverage-report)
   :official_mcp_comparison
   {:official_strengths ["Native Figma canvas writes through official MCP product APIs."
                         "Code Connect-aware context and code generation inside Figma's product surface."
                         "Make resources, FigJam generation, and hosted Remote MCP ergonomics."]
    :fighorse_strengths ["Self-hosted CLI-first pipeline under 1PL."
                         "Full public REST coverage with transparent operation registry."
                         "AI self-discovery, local experience learning, asset manifests, and reproducible visual feedback loops."
                         "Separate Figma write and local filesystem write safety controls."]
    :unsupported_by_public_rest api-coverage/official-mcp-only-capabilities
    :recommended_setup
    "Use both fighorse and the official Figma Remote MCP together. fighorse handles design-to-code read workflows; official MCP handles canvas writes, Code to Canvas, and Code Connect."}
   :complementary_mcp_servers
   [{:name "figma-official"
     :purpose "Native canvas writes, Code to Canvas, Code Connect, and product-only Figma capabilities."
     :remote_url "https://mcp.figma.com/mcp"
     :transport "http"
     :auth "OAuth via Figma account"
     :pricing "Beta: free. Future: usage-based paid feature (per Figma docs)."
     :seat_requirements "Full seat for write to shared files; Dev seat read-only outside drafts."
     :when_to_use ["Write directly to Figma canvas"
                    "Code to Canvas (push running UI into Figma as editable layers)"
                    "Code Connect automatic mapping"
                    "FigJam generation"
                    "Make resources"]
     :when_not_to_use "Design-to-code replication, asset export with manifests, visual audit, or local experience learning — use fighorse instead."}]
   :experience_loop
   {:store_path (experience/experience-path)
    :store (experience/store-info)
    :schema "Call get_experience_schema or fighorse experience schema for the versioned JSONL contract."
    :before_work "Call list_experiences or fighorse experience summary with platform/asset_format filters before implementing."
    :after_work "Call record_experience or fighorse experience add whenever a reusable mismatch, asset rule, platform rule, or workflow fix is learned."
    :compatibility "Experience records are append-only JSONL. Readers must ignore unknown fields so future fighorse versions remain compatible."}
   :learned_experience (experience/guidance :limit 5)
   :ai_contract (guidance/ai-contract)
   :output_locations (guidance/output-location-guidance)
   :recommended_workflow
   [{:step 1
     :tool "discover_fighorse"
     :reason "Learn available tools and contracts without external instructions."}
    {:step 2
     :tool "check_fighorse_ready"
     :reason "Verify local setup. If auth.has_token is false, prompt the user to run fighorse auth login --token <FIGMA_TOKEN> before calling Figma APIs."}
    {:step 3
     :tool "list_experiences"
     :reason "Load reusable local lessons before repeating known layout, typography, asset, or platform mistakes."}
    {:step 4
     :tool "parse_figma_url"
     :reason "Extract file_key and node_id from a pasted Figma URL when needed."}
    {:step 5
     :tool "get_design_package"
     :reason "Fetch compact structure, screenshots, tokens, platform guidance, learned experience, asset export plan, and implementation hints in one call."}
    {:step 6
     :action "If the target is a CANVAS/page/user flow or contains many children, narrow to exact frame/screen nodes before coding."
     :reason "Whole-flow pages are context for navigation, not a single UI surface to implement directly."}
    {:step 7
     :action "Ask the developer for missing platform/framework or asset format before implementation."
     :reason "Platform and asset format change typography, density, vector/raster export, and build-pipeline choices."}
    {:step 8
     :action "Export assets into a project-local or fighorse-managed directory with manifest enabled."
     :reason "Reasonable output locations avoid permission failures and make generated files discoverable by AI tools and build scripts."}
    {:step 9
     :action "Implement from the design package."
     :reason "Use screenshots for visual fidelity, context for layout, tokens for styling, and assets for image fills."}
    {:step 10
     :action "Run the implementation, capture screenshots, compare, and fix overlap/clipping/typography before finalizing."
     :reason "Real app screenshots catch container stacking, system chrome, compact typography, and localization issues."}
    {:step 11
     :tool "visual_audit"
     :reason "After implementation screenshots exist, structure fidelity checks and reusable experience suggestions."}
    {:step 12
     :tool "record_experience"
     :reason "Persist reusable lessons so the next AI client can self-learn from this run without a long prompt."}]
   :mcp
   {:transports
    {:http {:url "http://127.0.0.1:9449/mcp"
            :requires "Run the installed local service once; clients should reuse it instead of spawning stdio processes."}
     :stdio {:command "fighorse"
             :args ["mcp" "serve" "--transport" "stdio"]
             :env {:FIGHORSE_MCP_MODE "readonly"
                   :FIGHORSE_MCP_LOCAL_WRITE "allow"}}
     :sse {:command "fighorse"
           :args ["mcp" "serve" "--transport" "sse" "--host" "127.0.0.1" "--port" "9449"]
           :url "http://127.0.0.1:9449/sse"
           :env {:FIGHORSE_MCP_MODE "readonly"
                 :FIGHORSE_MCP_LOCAL_WRITE "allow"}}}
    :local_write {:env "FIGHORSE_MCP_LOCAL_WRITE=allow"
                  :allowed_roots ["./.fighorse/exports" "./assets/fighorse" "~/.fighorse/exports"]
                  :default "deny unless enabled by install-generated MCP configs"}
    :default_mode "readonly"
    :write_mode "Set FIGHORSE_MCP_MODE=write only when the AI client is allowed to mutate Figma resources."
    :self_discovery_tools ["discover_fighorse" "check_fighorse_ready" "parse_figma_url" "get_replicate_workflow" "get_experience_schema" "list_experiences"]
    :learning_tools ["get_experience_schema" "list_experiences" "record_experience"]
    :replication_tools ["get_design_package" "get_design_context" "get_screenshot" "export_images" "export_component" "download_image_fills" "get_tokens" "visual_audit" "get_project_playbook"]
    :resources ["fighorse://capabilities" "fighorse://coverage" "fighorse://workflow/design-replication" "fighorse://experience/summary"]
    :prompts ["fighorse_design_replication" "fighorse_api_coverage"]
     :complementary_servers
     [{:name "figma-official"
       :url "https://mcp.figma.com/mcp"
       :transport "http"
       :auth "OAuth"
       :purpose "Canvas writes, Code to Canvas, Code Connect"
       :pricing_note "Free during beta; will become usage-based paid"}]}
   :cli
   {:self_discovery_commands
    ["fighorse quickstart \"<figma-frame-url>\" --format json"
     "fighorse discover --format json"
     "fighorse doctor --format json"
     "fighorse install status"
     "fighorse install project"
     "fighorse smoke <figma-url>"
     "fighorse url parse <figma-url>"
     "fighorse experience summary --platform <target-platform> --asset-format <asset-format>"
     "fighorse experience add --summary <issue-pattern> --lesson <generalized-lesson> --platform <target-platform> --asset-format <asset-format>"
     "fighorse design package <figma-url> --platform <target-platform> --asset-format <asset-format> --max-tokens 8000"
     "fighorse mcp config --client cursor --transport http"]
    :install_commands
    ["fighorse install home"
     "fighorse install auth --apply"
     "fighorse install binary --source <path-to-fighorse-binary> --apply"
     "fighorse install project"
     "fighorse install client --client cursor"
     "fighorse install client --client cursor --apply"
     "fighorse install client --client codex"
     "fighorse install client --client codex --apply"
     "fighorse install client --client kimi --apply"
     "fighorse install client --client claude"
     "fighorse install client --client opencode"
     "fighorse install service --service launchd --apply"
     "fighorse install skill --clients cursor,codex,kimi --apply"
     "fighorse install --default --apply"
     "fighorse install --path ~/.local/bin --apply"
     "fighorse install --default --mode service --clients cursor,codex,kimi --apply"]}
   :auth
   {:required_for_figma_api true
    :env ["FIGMA_TOKEN" "FIGMA_API_KEY"]
    :local_config "fighorse auth login --token <FIGMA_TOKEN>"
    :missing_token_ai_prompt "fighorse needs a Figma Personal Access Token before it can read Figma files. Please run `fighorse auth login --token <FIGMA_TOKEN>` or set FIGMA_TOKEN, then ask me to retry."
    :safety "Do not commit tokens. Pass tokens through environment variables or local config only."}
   :quality_rules
   ["Use screenshot output as the visual source of truth."
    "Before implementation, review relevant local lessons with list_experiences or fighorse experience summary."
    "Before implementation, know the target platform/framework and asset format; if not supplied, ask the developer instead of guessing."
    "Use compact context for hierarchy, layout, spacing, text, and component clues."
    "Use tokens for colors, typography, spacing, and shadows."
    "Map typography explicitly: font family, font size, font weight, line height, and letter spacing."
    "When implementation details conflict, prioritize screenshots, then explicit tokens, then compact tree metadata."
    "If a required asset URL is missing, call get_image_fills, export_images, export_component, download_image_fills, image export, component export, or asset download before guessing."
    "Use --manifest for exported slices/assets when another AI tool or build script needs to discover generated files without extra instructions."
    "Store exports in ./.fighorse/exports for scratch work, ./assets/fighorse or the app resource directory for packaged assets, or ~/.fighorse/exports for cross-project scratch data."
    "MCP export tools require FIGHORSE_MCP_LOCAL_WRITE=allow and reject paths outside ./.fighorse/exports, ./assets/fighorse, and ~/.fighorse/exports."
    "Do not write generated exports to protected system paths, dependency caches, or hard-to-discover temp locations unless the developer explicitly asks."
    "Use a visual debug loop: implement, build/run, capture screenshot, compare with Figma, then fix overlap, clipping, status bars, and compact typography."
    "Inspect repeated components and child nodes individually when a whole-screen package is ambiguous."
    "If the selected target is a CANVAS, page, user flow, or contains many screen children, ask for the exact frame/screen or inspect the tree before implementing."
    "After fixing a reusable mismatch, record it with record_experience or fighorse experience add so future runs learn automatically."]})

(defn workflow []
  {:kind "fighorse.replicate-workflow.v1"
   :goal "Recreate a Figma design in code with high visual fidelity."
   :default_call
   {:tool "get_design_package"
    :arguments {:figma_url "<paste Figma URL>"
                :platform "<target platform/framework, ask developer if unknown>"
                :asset_format "<asset format, ask developer if unknown>"
                :depth 2
                :max_tokens 8000
                :include_screenshot true
                :include_assets true}}
   :steps
   ["Call list_experiences with platform/asset_format filters to load local lessons before implementing."
    "Call get_design_package with the pasted Figma URL."
    "If platform/framework or asset format is unknown, ask the developer before choosing implementation rules; png is only a render fallback, not a silent product decision."
    "Inspect target, screenshots, context, tokens, and assets."
    "Inspect important child nodes/components individually when layout, typography, or asset treatment is ambiguous."
    "Export local slices/components with manifest=true for icons, image fills, controls, and ambiguous visual details into ./.fighorse/exports, ./assets/fighorse, or another developer-approved directory."
    "Map Figma frames to app components and layout containers."
    "Implement styling from tokens and compact tree dimensions/layout."
    "Use screenshots to compare spacing, typography, colors, and hierarchy."
    "Run the project's normal tests/build after implementation."
    "When possible, run the app, capture screenshots, compare against Figma, and fix overlap/clipping/typography before finalizing."
    "Call record_experience after a reusable lesson is discovered so the next run can self-learn."]
   :avoid
   ["Do not ask the user to manually extract file_key or node_id from a normal Figma URL."
    "Do not choose platform/framework or asset format silently when the developer has not specified it."
    "Do not ignore screenshots when they are available."
    "Do not invent image assets when get_image_fills or screenshots provide references."
    "Do not write exports to protected system paths, dependency caches, or hidden locations that the app/build cannot easily use."
    "Do not assume a single typography scale applies to compact and full-size components."
    "Do not discard lessons from screenshot debugging; persist reusable findings through the experience interface."]})

(defn doctor []
  (let [{:keys [token config-path proxy]} (config/load-config)
        bun-version (some-> js/globalThis .-Bun .-version)
        has-token? (boolean (seq token))
        local-write? (config/mcp-local-write-enabled?)
        mcp-service (mcp-service-status)
        stale-lock? (and (:lock_present mcp-service)
                         (not (:running mcp-service)))]
    {:kind "fighorse.doctor.v1"
     :runtime {:name "bun"
               :version bun-version
               :ok (boolean bun-version)}
     :mcp {:mode (:mcp-mode (config/load-config))
           :write_enabled (config/mcp-write-enabled?)
           :local_write_enabled (config/mcp-local-write-enabled?)
           :local_write_env "FIGHORSE_MCP_LOCAL_WRITE=allow"}
     :install {:home (:fighorse-home (config/load-config))
               :output_locations (guidance/output-location-guidance)
               :next_step "Run fighorse install status first. Install commands generate artifacts by default; pass --apply only when you want fighorse to mutate detected CLI, MCP service, AI client, and skill locations."}
     :experience {:store_path (experience/experience-path)
                  :store (experience/store-info)
                  :schema_version experience/schema-version
                  :records (count (experience/read-all))
                  :next_step "Call fighorse experience summary or MCP list_experiences before implementing a Figma design."}
     :auth {:has_token (boolean (seq token))
            :config_path config-path
            :env_token_present (boolean (or (seq (.-FIGMA_TOKEN js/process.env))
                                            (seq (.-FIGMA_API_KEY js/process.env))))
            :required_for_figma_api true
            :setup_command "fighorse auth login --token <FIGMA_TOKEN>"
            :missing_token_ai_prompt "fighorse needs a Figma Personal Access Token before it can read Figma files. Please run `fighorse auth login --token <FIGMA_TOKEN>` or set FIGMA_TOKEN, then ask me to retry."}
     :checks [{:id "token"
               :ok has-token?
               :message (if has-token?
                          "Figma token is configured."
                          "Figma token is missing; Figma API calls will fail.")
               :next_step "Run fighorse auth login --token <FIGMA_TOKEN> or set FIGMA_TOKEN for one command."}
              {:id "mcp_service"
               :ok (:running mcp-service)
               :message (if (:running mcp-service)
                          "Local MCP service appears to have an active singleton owner."
                          "Local MCP service is not running. This is fine for CLI-only mode.")
               :next_step "For AI clients, run fighorse install --default --mode service --clients cursor,codex,kimi --apply."}
              {:id "mcp_repeated_handshake"
               :ok true
               :message "The /mcp endpoint is expected to create a fresh stateless transport/server per request, so Codex-style repeated initialize handshakes stay valid."
               :next_step "If a client reports text/plain during initialize, restart the installed fighorse service and verify /mcp implementation notes in AGENTS.md."}
              {:id "local_write"
               :ok local-write?
               :message (if local-write?
                          "MCP local file export is enabled."
                          "MCP local file export is disabled by default.")
               :next_step "Set FIGHORSE_MCP_LOCAL_WRITE=allow only when the client may write under ./.fighorse/exports, ./assets/fighorse, or ~/.fighorse/exports."}
              {:id "stale_singleton_lock"
               :ok (not stale-lock?)
               :message (if stale-lock?
                          "A stale MCP singleton lock was found."
                          "No stale MCP singleton lock detected.")
               :next_step (str "Remove " (:lock_file mcp-service) " only after confirming no fighorse MCP service is running.")}]
     :mcp_service mcp-service
     :setup (setup-guidance)
     :troubleshooting {:broad_canvas_target "If diagnostics mention CANVAS, page, or user-flow target, copy a link to a specific frame, component, or group."
                       :token_missing "Run fighorse auth login --token <FIGMA_TOKEN>. AI clients should surface this exact command when auth.has_token is false."
                       :export_path_rejected "Use ./.fighorse/exports, ./assets/fighorse, or ~/.fighorse/exports. MCP also requires FIGHORSE_MCP_LOCAL_WRITE=allow."
                       :mcp_unexpected_content_type "Codex/Cursor should target http://127.0.0.1:9449/mcp. The handler must return MCP JSON/SSE for every initialize request, including repeats."
                       :quickstart "Run fighorse quickstart \"<figma-frame-url>\" for the shortest public onboarding path."}
     :proxy {:configured (boolean proxy)
             :value proxy}
     :recommended_next_step (if has-token?
                              "Call list_experiences, then get_design_package with a Figma URL."
                              "Set FIGMA_TOKEN or run fighorse auth login --token <FIGMA_TOKEN>.")}))

(defn mcp-config
  [& {:keys [client transport port command]
      :or {client "generic"
           transport "http"
           port 9449
           command "fighorse"}}]
  (let [stdio {:command command
               :args ["mcp" "serve" "--transport" "stdio"]
               :env {:FIGMA_TOKEN "<FIGMA_TOKEN>"
                     :FIGHORSE_HOME "~/.fighorse"
                     :FIGHORSE_MCP_MODE "readonly"
                     :FIGHORSE_MCP_LOCAL_WRITE "allow"}}
        sse {:command command
             :args ["mcp" "serve" "--transport" "sse" "--host" "127.0.0.1" "--port" (str port)]
             :url (str "http://127.0.0.1:" port "/sse")
             :env {:FIGMA_TOKEN "<FIGMA_TOKEN>"
                   :FIGHORSE_HOME "~/.fighorse"
                   :FIGHORSE_MCP_MODE "readonly"
                   :FIGHORSE_MCP_LOCAL_WRITE "allow"}}
        http {:transport "http"
              :url (str "http://127.0.0.1:" port "/mcp")}]
    {:kind "fighorse.mcp-config.v1"
     :client client
     :transport transport
     :recommended_tool_order ["discover_fighorse" "check_fighorse_ready" "list_experiences" "get_design_package" "record_experience"]
     :config (case transport
               "http" http
               "sse" sse
               stdio)
     :examples
     {:cursor {:mcpServers {:fighorse (case transport
                                        "http" {:url (:url http)}
                                        "sse" {:url (:url sse)}
                                        stdio)}}
      :generic {:fighorse (case transport
                            "http" http
                            "sse" sse
                            stdio)}}}))

(defn manifest->markdown [m]
  (str "# " (:name m) " " (:version m) "\n\n"
       (:purpose m) "\n\n"
       "## Primary Use Case\n\n"
       (:primary_use_case m) "\n\n"
       "## Setup Requirements\n\n"
       "- Figma API calls require a Figma Personal Access Token.\n"
       "- Recommended setup: `fighorse auth login --token <FIGMA_TOKEN>`.\n"
       "- One-shot setup: `FIGMA_TOKEN=<FIGMA_TOKEN> fighorse quickstart \"<figma-frame-url>\"`.\n"
       "- If `check_fighorse_ready` reports `auth.has_token=false`, ask the user to configure the token before calling Figma API tools.\n\n"
       "## Recommended Workflow\n\n"
       (str/join "\n"
                 (map (fn [{:keys [step tool action reason]}]
                        (str step ". " (or tool action) " - " reason))
                      (:recommended_workflow m)))
       "\n\n## CLI Self Discovery\n\n"
       (str/join "\n" (map #(str "- `" % "`") (get-in m [:cli :self_discovery_commands])))
       "\n\n## Experience Loop\n\n"
       "Before implementation, run `fighorse experience summary --platform <platform> --asset-format <format>`. "
       "After fixing a reusable mismatch, run `fighorse experience add --summary <issue> --lesson <lesson> --platform <platform>`.\n"
       "\n"))

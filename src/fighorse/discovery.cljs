(ns fighorse.discovery
  "Self-description payloads for AI tools and MCP clients."
  (:require [clojure.string :as str]
            [fighorse.api.coverage :as api-coverage]
            [fighorse.config :as config]
            [fighorse.experience :as experience]
            [fighorse.guidance :as guidance]))

(def version "0.1.0")

(defn manifest []
  {:kind "fighorse.discovery.v1"
   :name "fighorse"
   :version version
   :purpose "Provide Figma design context, screenshots, tokens, and implementation hints to AI coding tools."
   :primary_use_case "Given a Figma URL, produce enough structured context for an AI coding tool to recreate the selected design."
   :production_defaults
   {:mcp_mode "readonly"
    :mcp_local_write "set FIGHORSE_MCP_LOCAL_WRITE=allow only for safe local asset exports"
    :fighorse_home "~/.fighorse"
    :global_experience "~/.fighorse/experience/global.jsonl"
    :project_experience "./.fighorse/experience.jsonl after fighorse install project"
    :default_design_package {:depth 2
                             :max_tokens 8000
                             :include_screenshot true
                             :include_assets false
                             :platform "ask-developer-if-unspecified"
                             :asset_format "ask-developer-if-unspecified; png is only the render fallback"}
    :smoke_test "fighorse smoke <figma-url>"}
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
    :fighorse_strengths ["MIT self-hosted CLI-first pipeline."
                         "Full public REST coverage with transparent operation registry."
                         "AI self-discovery, local experience learning, asset manifests, and reproducible visual feedback loops."
                         "Separate Figma write and local filesystem write safety controls."]
    :unsupported_by_public_rest api-coverage/official-mcp-only-capabilities}
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
     :tool "list_experiences"
     :reason "Load reusable local lessons before repeating known layout, typography, asset, or platform mistakes."}
    {:step 3
     :tool "parse_figma_url"
     :reason "Extract file_key and node_id from a pasted Figma URL when needed."}
    {:step 4
     :tool "get_design_package"
     :reason "Fetch compact structure, screenshots, tokens, platform guidance, learned experience, asset export plan, and implementation hints in one call."}
    {:step 5
     :action "If the target is a CANVAS/page/user flow or contains many children, narrow to exact frame/screen nodes before coding."
     :reason "Whole-flow pages are context for navigation, not a single UI surface to implement directly."}
    {:step 6
     :action "Ask the developer for missing platform/framework or asset format before implementation."
     :reason "Platform and asset format change typography, density, vector/raster export, and build-pipeline choices."}
    {:step 7
     :action "Export assets into a project-local or fighorse-managed directory with manifest enabled."
     :reason "Reasonable output locations avoid permission failures and make generated files discoverable by AI tools and build scripts."}
    {:step 8
     :action "Implement from the design package."
     :reason "Use screenshots for visual fidelity, context for layout, tokens for styling, and assets for image fills."}
    {:step 9
     :action "Run the implementation, capture screenshots, compare, and fix overlap/clipping/typography before finalizing."
     :reason "Real app screenshots catch container stacking, system chrome, compact typography, and localization issues."}
    {:step 10
     :tool "visual_audit"
     :reason "After implementation screenshots exist, structure fidelity checks and reusable experience suggestions."}
    {:step 11
     :tool "record_experience"
     :reason "Persist reusable lessons so the next AI client can self-learn from this run without a long prompt."}]
   :mcp
   {:transports
    {:stdio {:command "fighorse"
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
    :prompts ["fighorse_design_replication" "fighorse_api_coverage"]}
   :cli
   {:self_discovery_commands
    ["fighorse discover --format json"
     "fighorse doctor --format json"
     "fighorse install status"
     "fighorse install project"
     "fighorse smoke <figma-url>"
     "fighorse url parse <figma-url>"
     "fighorse experience summary --platform <target-platform> --asset-format <asset-format>"
     "fighorse experience add --summary <issue-pattern> --lesson <generalized-lesson> --platform <target-platform> --asset-format <asset-format>"
     "fighorse design package <figma-url> --platform <target-platform> --asset-format <asset-format> --max-tokens 8000"
     "fighorse mcp config --client cursor --transport stdio"]
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
     "fighorse install all --clients cursor,codex,kimi --source <path-to-fighorse-binary> --apply"]}
   :auth
   {:required_for_figma_api true
    :env ["FIGMA_TOKEN" "FIGMA_API_KEY"]
    :local_config "fighorse auth login --token <FIGMA_TOKEN>"
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
        bun-version (some-> js/globalThis .-Bun .-version)]
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
                                            (seq (.-FIGMA_API_KEY js/process.env))))}
     :proxy {:configured (boolean proxy)
             :value proxy}
     :recommended_next_step (if (seq token)
                              "Call list_experiences, then get_design_package with a Figma URL."
                              "Set FIGMA_TOKEN or run fighorse auth login --token <FIGMA_TOKEN>.")}))

(defn mcp-config
  [& {:keys [client transport port command]
      :or {client "generic"
           transport "stdio"
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
                   :FIGHORSE_MCP_LOCAL_WRITE "allow"}}]
    {:kind "fighorse.mcp-config.v1"
     :client client
     :transport transport
     :recommended_tool_order ["discover_fighorse" "check_fighorse_ready" "list_experiences" "get_design_package" "record_experience"]
     :config (if (= "sse" transport) sse stdio)
     :examples
     {:cursor {:mcpServers {:fighorse (if (= "sse" transport)
                                        {:url (:url sse)}
                                        stdio)}}
      :generic {:fighorse (if (= "sse" transport) sse stdio)}}}))

(defn manifest->markdown [m]
  (str "# " (:name m) " " (:version m) "\n\n"
       (:purpose m) "\n\n"
       "## Primary Use Case\n\n"
       (:primary_use_case m) "\n\n"
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

(ns fighorse.design-package
  "High-level Figma design package for AI replication workflows."
  (:require [clojure.string :as str]
            [fighorse.api.files :as files-api]
            [fighorse.compact :as compact]
            [fighorse.experience :as experience]
            [fighorse.figma :as figma]
            [fighorse.guidance :as guidance]
            [fighorse.tokens :as tokens]
            [fighorse.utils.url :as url]))

(defn resolve-source
  [& {:keys [figma-url file-key node-id]}]
  (let [parsed (when figma-url (url/parse-figma-url figma-url))
        resolved-file-key (or file-key (:file_key parsed))
        resolved-node-id (or node-id (:node_id parsed))]
    (cond-> {:figma_url figma-url
             :file_key resolved-file-key
             :node_id resolved-node-id
             :parsed parsed}
      (nil? figma-url) (dissoc :figma_url :parsed))))

(defn- require-source! [source]
  (when-not (seq (:file_key source))
    (throw (js/Error. "A Figma URL or file_key is required")))
  source)

(defn- file-summary [data]
  (select-keys data [:name :lastModified :thumbnailUrl :version :role :editorType :linkAccess]))

(defn- truncated? [node]
  (or (= true (:truncated node))
      (some truncated? (:children node))))

(defn- token-counts [grouped]
  (into {} (map (fn [[k v]] [k (count v)]) grouped)))

(defn- normalize-target [value default]
  (let [value (some-> value str str/trim str/lower-case)]
    (if (str/blank? value) default value)))

(defn- missing-target? [value]
  (str/blank? (some-> value str str/trim)))

(def ^:private render-asset-formats #{"png" "jpg" "jpeg" "svg" "pdf"})

(defn- render-asset-format [asset-format]
  (let [asset-format (normalize-target asset-format "png")]
    (case asset-format
      "jpeg" "jpg"
      (if (contains? render-asset-formats asset-format) asset-format "png"))))

(defn- implementation-target [platform asset-format]
  (let [platform (normalize-target platform "unspecified")
        asset-format-missing (missing-target? asset-format)
        asset-format (normalize-target asset-format "png")
        render-format (render-asset-format asset-format)]
    {:platform platform
     :asset_format asset-format
     :render_asset_format render-format
     :ask_developer_when
     (cond-> []
       (= "unspecified" platform)
       (conj "Target platform/framework is not specified. Ask whether the output should be Android Compose, iOS SwiftUI/UIKit, Web, Flutter, React Native, etc.")
       asset-format-missing
       (conj "Asset export format was not specified. Ask whether slices should be png, jpg, svg, pdf, or app-specific vector assets before exporting.")
       (not= render-format asset-format)
       (conj (str "Requested asset format `" asset-format "` is not supported by Figma node rendering. Use `" render-format "` for rendered slices, or download original image fills when the source content type provides that format.")))
     :rules
     (cond-> ["Do not silently choose a different platform or asset format than the developer requested."
              "If Figma metadata is insufficient for a platform-specific decision, ask the developer instead of inventing a rule."
              "Record platform assumptions in the implementation notes or generated code comments when they affect fidelity."]
       (str/includes? platform "android")
       (into ["For Android, prefer density-independent layout values and export bitmap slices as png unless vector/svg is explicitly requested."
              "For Jetpack Compose, map textStyle fontSize/lineHeight/fontWeight explicitly and avoid relying on default Material typography."])
       (str/includes? platform "ios")
       (into ["For iOS, ask whether SwiftUI or UIKit is required and prefer pdf/svg vector assets when the app pipeline supports them."
              "Map Figma textStyle to explicit font, size, weight, and line height instead of relying on platform defaults."])
       (or (str/includes? platform "web") (str/includes? platform "react"))
       (into ["For web, prefer svg for vector icons and png/webp for raster imagery according to the target build pipeline."
              "Map Figma textStyle to CSS font-family, font-size, font-weight, line-height, and letter-spacing."]))}))

(defn- fidelity-workflow [platform asset-format]
  {:goal "Turn Figma data into a high-fidelity implementation through iterative visual verification."
   :platform (normalize-target platform "unspecified")
   :asset_format (normalize-target asset-format "png")
   :steps
   ["Start with the whole target frame to understand screen hierarchy and major spacing."
    "Inspect key child nodes/components for textStyle, dimensions, fills, strokes, effects, and layout constraints."
    "Export local slices for ambiguous icons, images, controls, cards, and repeated components before approximating them."
    "Implement the screen using explicit typography, spacing, and asset references from Figma context/tokens."
    "Build and run the target app when possible, capture an app screenshot, and compare it against the Figma screenshot."
    "Fix overlap, clipping, wrong font size/line-height, wrong component stacking, and status/navigation bar conflicts before moving on."
    "Repeat the loop for representative screens and component states."]
   :attention_checks
   ["Check whether containers with multiple children are implemented with the correct layout primitive instead of stacking children accidentally."
    "Check compact cards, message bubbles, and list rows separately from full-size versions; they often use different typography and line height."
    "Check real device/system chrome against Figma status bars and navigation bars to avoid double-rendered bars."
    "Check long localized strings and generated names for clipping or overlap."
    "If a visual mismatch cannot be resolved from the package, ask the developer which tradeoff is acceptable."]})

(defn- asset-export-plan [file-key node-id platform asset-format]
  (let [asset-format (normalize-target asset-format "png")
        render-format (render-asset-format asset-format)
        platform (normalize-target platform "unspecified")]
    {:preferred_format asset-format
     :render_format render-format
     :platform platform
     :output_policy (guidance/output-location-guidance)
     :cli_examples
     (cond-> [(str "fighorse image export " file-key " --ids " (or node-id "<node-id>") " --format " render-format " --dir ./.fighorse/exports --manifest")
              (str "fighorse component export " file-key " --ids <component-node-id> --format " render-format " --dir ./assets/fighorse --manifest")
              (str "fighorse asset download " file-key " --dir ./assets/fighorse --manifest")]
       (= "unspecified" platform)
       (conj "Ask the developer for the target platform before choosing final asset format or density rules."))
     :mcp_tools
     [{:tool "export_images"
       :when "Need local frame/node slices for implementation or visual comparison."}
      {:tool "export_component"
       :when "Need a local image for a Figma component/control node."}
      {:tool "download_image_fills"
       :when "Need original raster image fills from the Figma file."}]}))

(defn- node-bounds [node]
  (or (:absoluteBoundingBox node)
      (:absoluteRenderBounds node)
      (:size node)))

(defn- all-nodes [node]
  (tree-seq #(seq (:children %)) :children node))

(defn- screen-candidates [target]
  (->> (all-nodes target)
       (filter #(contains? #{"FRAME" "COMPONENT" "INSTANCE" "SECTION"} (:type %)))
       (map (fn [node]
              (let [bounds (node-bounds node)]
                {:id (:id node)
                 :name (:name node)
                 :type (:type node)
                 :width (:width bounds)
                 :height (:height bounds)
                 :renderable (boolean (:id node))
                 :reason (if (and (:width bounds) (:height bounds)
                                  (> (:width bounds) 200)
                                  (> (:height bounds) 200))
                           "large candidate frame/component"
                           "structural candidate")})))
       (remove #(str/blank? (:id %)))
       (take 20)
       vec))

(defn- component-candidates [target]
  (->> (all-nodes target)
       (filter #(contains? #{"COMPONENT" "COMPONENT_SET" "INSTANCE"} (:type %)))
       (map (fn [node]
              {:id (:id node)
               :name (:name node)
               :type (:type node)
               :component_id (:componentId node)
               :component_set_id (:componentSetId node)
               :bounds (node-bounds node)}))
       (take 30)
       vec))

(defn- missing-font-diagnostics [target]
  (let [text-nodes (filter #(= "TEXT" (:type %)) (all-nodes target))
        missing (->> text-nodes
                     (filter #(str/blank? (get-in % [:style :fontFamily])))
                     (map #(select-keys % [:id :name :type]))
                     (take 20)
                     vec)]
    {:checked_text_nodes (count text-nodes)
     :missing_font_family_count (count missing)
     :examples missing
     :ai_guidance (if (seq missing)
                    "Some text nodes do not expose fontFamily in the compact package. Inspect raw node details or ask the developer about font availability before approximating typography."
                    "Font family metadata is present for inspected text nodes.")}))

(defn- token-confidence [grouped-tokens]
  (let [counts (token-counts grouped-tokens)
        total (reduce + 0 (vals counts))]
    {:status (cond
               (zero? total) "missing"
               (< total 4) "low"
               :else "usable")
     :counts counts
     :ai_guidance "Use token values when present, but verify typography and raster assets against screenshots."}))

(defn- implementation-risk-checklist [target platform asset-format]
  (cond-> ["Check selected target scope before coding; CANVAS or flow nodes should be narrowed to frames."
           "Check screenshot fidelity after implementation, not only structured JSON."
           "Check repeated siblings, compact cards, and list rows for overlap or wrong layout primitives."
           "Check asset export manifest before referencing local files."]
    (str/blank? (some-> platform str str/trim))
    (conj "Target platform is unspecified; ask before choosing framework, density, or native controls.")
    (str/blank? (some-> asset-format str str/trim))
    (conj "Asset format is unspecified; ask before exporting final slices.")
    (= "CANVAS" (:type target))
    (conj "Current target is a CANVAS/page; use screen_candidates to pick exact frames.")))

(defn- implementation-hints [platform asset-format]
  {:intent "Use this package to recreate the selected Figma design in code."
   :priority_order ["screenshots" "learned_experience" "tokens" "context" "assets" "file metadata"]
   :layout "Use context.children, dimensions, layout, padding, itemSpacing, and textStyle to build component hierarchy."
   :visuals "Use tokens and node fills/strokes/effects for colors, borders, shadows, typography, and spacing."
   :assets "Use screenshots for verification and image_fills for raster assets when present."
   :platform "Use implementation_target.platform when present. If it is unspecified, ask the developer before choosing a platform/framework."
   :asset_format "Use implementation_target.asset_format for exported slices/assets. If the app pipeline requires another format, ask before converting."
   :output_locations "Use asset_export_plan.output_policy. Prefer ./.fighorse/exports for temporary slices and ./assets/fighorse or app resource directories for packaged assets. Ask before writing elsewhere."
   :ai_contract (guidance/ai-contract)
   :quality_checks ["Compare final UI against screenshots."
                    "Preserve visible text content."
                    "If target.type is CANVAS or the target has many children, narrow to exact frame/screen nodes before implementing."
                    "Map font family, font size, line height, font weight, and letter spacing explicitly."
                    "Check compact components separately from full-size components."
                    "Run the app and capture screenshots when possible; fix overlap/clipping from real screenshots."
                    "Prefer exact token values over approximate colors."
                    "If context is truncated, request a lower-depth package or targeted node package."
                    "Export ambiguous assets with manifest enabled into a reasonable project-local or fighorse-managed directory."
                    "If platform, asset format, or fidelity tradeoffs are unclear, ask the developer instead of guessing."]
   :target (implementation-target platform asset-format)})

(defn- diagnostics [target compacted grouped-tokens screenshots assets platform asset-format]
  (let [image-count (count (:images screenshots))
        asset-count (count (or (get-in assets [:meta :images])
                               (:images assets)
                               {}))
        token-count-map (token-counts grouped-tokens)
        platform (normalize-target platform "unspecified")
        asset-format-missing (missing-target? asset-format)
        asset-format (normalize-target asset-format "png")
        render-format (render-asset-format asset-format)
        target-type (:type target)
        child-count (count (:children target))]
    {:status (if (and (pos? image-count)
                      (pos? (reduce + 0 (vals token-count-map))))
               "ready"
               "partial")
     :context_truncated (boolean (truncated? compacted))
     :screenshots {:requested (boolean screenshots)
                   :count image-count}
     :tokens {:categories (keys token-count-map)
              :counts token-count-map}
     :assets {:requested (boolean assets)
              :count asset-count}
     :warnings (cond-> []
                 (zero? image-count)
                 (conj "No screenshot URL was returned. Use get_screenshot or lower the target scope.")
                 (zero? (reduce + 0 (vals token-count-map)))
                 (conj "No design tokens were extracted. Inspect context fills/textStyle directly.")
                 (truncated? compacted)
                 (conj "Context was token-budget truncated. Request a smaller node or larger max_tokens for more detail.")
                 (= "unspecified" platform)
                 (conj "Target platform/framework was not specified. Ask the developer before choosing platform-specific typography, density, or asset rules.")
                 asset-format-missing
                 (conj "Asset export format was not specified. Ask the developer before choosing final slice or asset format.")
                 (not= render-format asset-format)
                 (conj (str "Asset format `" asset-format "` is not supported for rendered node slices. Use `" render-format "` for render exports, and use image fill downloads for original raster formats when available."))
                 (= "CANVAS" target-type)
                 (conj "Selected target is a CANVAS/page. Ask for the exact frame/screen node or inspect child frames before implementing.")
                 (> child-count 12)
                 (conj "Selected target has many direct children. Treat it as a flow/overview and narrow to representative frames before coding."))
     :production_note "Use status=ready as the normal AI replication path; handle status=partial by following warnings and next_tools."}))

(defn- package-base [source data target compacted grouped-tokens screenshots assets platform asset-format]
  (let [learned (experience/guidance :platform platform
                                      :asset-format asset-format
                                      :limit 6)]
    {:kind "fighorse.design-package.v1"
     :source (dissoc source :parsed)
     :file (file-summary data)
     :target (figma/node-summary target)
     :implementation_target (implementation-target platform asset-format)
     :screen_candidates (screen-candidates target)
     :component_candidates (component-candidates target)
     :ai_contract (guidance/ai-contract)
     :fidelity_workflow (fidelity-workflow platform asset-format)
     :asset_export_plan (asset-export-plan (:file_key source) (:node_id source) platform asset-format)
     :learned_experience learned
     :context compacted
     :tokens grouped-tokens
     :token_confidence (token-confidence grouped-tokens)
     :missing_font_diagnostics (missing-font-diagnostics target)
     :implementation_risk_checklist (implementation-risk-checklist target platform asset-format)
     :diagnostics (assoc (diagnostics target compacted grouped-tokens screenshots assets platform asset-format)
                         :experience {:store_path (get-in learned [:summary :store_path])
                                      :schema_version experience/schema-version
                                      :total_count (get-in learned [:summary :total_count])
                                      :returned_count (get-in learned [:summary :returned_count])
                                      :next_step "Review learned_experience before implementation and call record_experience after reusable fixes."})
     :implementation_hints (implementation-hints platform asset-format)
     :next_tools [{:tool "list_experiences"
                   :when "Need local lessons before implementing or when a mismatch repeats."}
                  {:tool "get_screenshot"
                   :when "Need a fresh render URL or a different scale/format."}
                  {:tool "export_images"
                   :when "Need local frame/node slices and a manifest for app implementation."}
                  {:tool "export_component"
                   :when "Need to export a control/component node as an image slice."}
                  {:tool "download_image_fills"
                   :when "Need original image fill assets with usable file extensions."}
                  {:tool "record_experience"
                   :when "After fixing a reusable overlap, clipping, typography, asset, platform, or workflow issue."}
                  {:tool "get_tokens"
                   :when "Need a token-only payload."}
                  {:tool "get_design_context"
                   :when "Need compact context without screenshots/assets."}]}))

(defn get-design-package
  [token & {:keys [figma-url file-key node-id depth max-tokens
                   include-screenshot include-assets screenshot-format
                   scale screenshot-limit platform asset-format]
            :or {depth 2
                 max-tokens 8000
                 screenshot-format "png"
                 scale 2
                 screenshot-limit 4}}]
  (let [source (require-source! (resolve-source :figma-url figma-url
                                                :file-key file-key
                                                :node-id node-id))
        depth (or depth 2)
        max-tokens (or max-tokens 8000)
        include-screenshot (not= false include-screenshot)
        include-assets (= true include-assets)
        screenshot-format (or screenshot-format "png")
        scale (or scale 2)
        screenshot-limit (or screenshot-limit 4)
        resolved-file-key (:file_key source)
        resolved-node-id (:node_id source)
        file-promise (if (seq resolved-node-id)
                       (files-api/get-file-nodes token resolved-file-key resolved-node-id :depth depth)
                       (files-api/get-file token resolved-file-key :depth depth))]
    (.then file-promise
           (fn [data]
             (let [target (figma/response->node data)
                   simplified (compact/simplify-tree target compact/default-extractors depth)
                   compacted (compact/compact target
                                             :extractors compact/default-extractors
                                             :max-depth depth
                                             :max-tokens max-tokens)
                 grouped-tokens (tokens/tokens-by-category (tokens/extract-tokens simplified))
                   screenshot-ids (figma/renderable-node-ids target resolved-node-id screenshot-limit)
                   screenshot-promise (if (and include-screenshot (seq screenshot-ids))
                                        (files-api/get-images token resolved-file-key screenshot-ids
                                                              :format screenshot-format
                                                              :scale scale)
                                        (js/Promise.resolve nil))
                   assets-promise (if include-assets
                                    (files-api/get-image-fills token resolved-file-key)
                                    (js/Promise.resolve nil))]
               (.then (js/Promise.all #js [screenshot-promise assets-promise])
                      (fn [results]
                        (let [screenshots (aget results 0)
                              assets (aget results 1)]
                          (cond-> (package-base source data target compacted grouped-tokens screenshots assets platform asset-format)
                            screenshots (assoc :screenshots {:format screenshot-format
                                                             :scale scale
                                                             :node_ids screenshot-ids
                                                             :images (:images screenshots)})
                            assets (assoc :assets {:image_fills (:meta assets)}))))))))))

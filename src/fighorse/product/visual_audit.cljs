(ns fighorse.product.visual-audit
  "Product-layer visual audit guidance for Figma implementation loops."
  (:require [clojure.string :as str]
            [fighorse.experience :as experience]
            [fighorse.guidance :as guidance]
            [fighorse.utils.url :as figma-url]))

(defn audit
  [& {:keys [figma-url screenshot-path platform asset-format notes]}]
  (let [parsed (when-not (str/blank? figma-url)
                 (figma-url/parse-figma-url figma-url))
        learned (experience/guidance :platform platform
                                     :asset-format asset-format
                                     :limit 6)]
    {:kind "fighorse.visual-audit.v1"
     :source {:figma_url figma-url
              :parsed parsed
              :implementation_screenshot screenshot-path}
     :target {:platform (or platform "unspecified")
              :asset_format (or asset-format "unspecified")}
     :status (if (str/blank? screenshot-path) "checklist-only" "ready-for-human-or-ai-comparison")
     :checklist ["Compare the implementation screenshot against the Figma render at the same viewport or device size."
                 "Check frame bounds, safe areas, status bars, navigation bars, and scroll behavior first."
                 "Check typography explicitly: font family, size, weight, line height, and letter spacing."
                 "Check repeated rows/cards for accidental stacking, overlap, clipping, or missing list containers."
                 "Check icons, raster fills, and component states against exported assets and manifests."
                 "Check long text and localization-sensitive labels for wrapping and overflow."
                 "Record reusable findings with record_experience or fighorse experience add."]
     :ai_guidance {:prompt "Use this audit checklist to produce a concise mismatch report: observed difference, likely cause, fix, and whether it should become reusable experience."
                   :next_tools ["get_design_package" "export_images" "download_image_fills" "record_experience"]}
     :learned_experience learned
     :notes notes
     :output_policy (guidance/output-location-guidance)}))

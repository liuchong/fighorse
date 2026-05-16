(ns fighorse.product.playbook
  "Project-level AI playbook assembled from fighorse guidance and local experience."
  (:require [fighorse.api.coverage :as coverage]
            [fighorse.experience :as experience]
            [fighorse.guidance :as guidance]))

(defn build
  [& {:keys [platform asset-format project-dir]}]
  (let [learned (experience/guidance :platform platform
                                     :asset-format asset-format
                                     :project-dir project-dir
                                     :limit 12)]
    {:kind "fighorse.project-playbook.v1"
     :target {:platform (or platform "unspecified")
              :asset_format (or asset-format "unspecified")
              :project_dir project-dir}
     :principles ["Use fighorse discovery before low-level tools."
                  "Use design packages for implementation context and screenshots."
                  "Export ambiguous assets with manifests into approved project-local roots."
                  "Run the target app and compare screenshots before declaring fidelity complete."
                  "Record transferable lessons after fixing visual, asset, or platform issues."]
     :official_api {:coverage (coverage/operation-summary)
                    :generic_cli "fighorse figma api <operationId> --params '{...}'"
                    :generic_mcp "Use figma_<operation_id_in_snake_case> tools for exact REST operations."}
     :ai_contract (guidance/ai-contract)
     :output_policy (guidance/output-location-guidance)
     :learned_experience learned
     :next_steps ["Call get_design_package for the target Figma URL."
                  "Call visual_audit after capturing an implementation screenshot."
                  "Call record_experience with reusable lessons."]}))

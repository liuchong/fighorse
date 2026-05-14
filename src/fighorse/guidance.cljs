(ns fighorse.guidance
  "Shared machine-readable guidance for AI clients using fighorse."
  (:require [fighorse.config :as config]))

(defn export-dirs []
  {:recommended_roots ["<project>/.fighorse/exports"
                       "<project>/assets/fighorse"
                       "~/.fighorse/exports"]
   :default_cli_dir "./.fighorse/exports"
   :rules ["Prefer a project-local directory when implementing app UI so generated files are easy to reference from source code."
           "Use ./.fighorse/exports for temporary slices, screenshots, manifests, and debug assets."
           "Use ./assets/fighorse or the app's normal resource directory only for assets that should be committed or packaged."
           "Use ~/.fighorse/exports for cross-project scratch data or when no project directory is available."
           "Do not write exports to /, /System, /usr, /bin, /var, protected app directories, dependency caches, or arbitrary hidden system locations."
           "MCP local file export tools require FIGHORSE_MCP_LOCAL_WRITE=allow and still validate that outputs remain inside the allowed export roots."
           "If the destination is unclear or write access fails, ask the developer for an output directory instead of guessing."
           "Always request manifest=true or pass --manifest for exported assets so AI tools and build scripts can discover generated files."]})

(defn ai-contract []
  {:kind "fighorse.ai-contract.v1"
   :must ["Call discover_fighorse or `fighorse discover --format json` before using low-level tools."
          "Call list_experiences or `fighorse experience summary` before implementation and after a mismatch."
          "Use get_design_package or `fighorse design package` as the default source of truth for implementation."
          "Pass platform and asset_format explicitly; if either is unknown, ask the developer before implementing."
          "Treat learned experiences as transferable patterns, not fixed rules for one project, design, or platform; adapt them to the current target."
          "Apply platform-specific lessons only when the current target platform matches or the developer confirms the analogy."
          "If the selected target is a CANVAS/page/user flow or has many children, narrow to exact frame/screen nodes before coding."
          "Use screenshots as the visual source of truth, then learned_experience, explicit typography, tokens, compact context, and assets."
          "Export ambiguous icons, controls, image fills, repeated cards, and visual details before approximating them."
          "Store exported files in ./.fighorse/exports, ./assets/fighorse, or ~/.fighorse/exports, and include a manifest."
          "For MCP asset export, ensure FIGHORSE_MCP_LOCAL_WRITE=allow is enabled; do not ask for unrestricted filesystem access."
          "Run/build the target app when possible, capture screenshots, compare against Figma, and fix overlap, clipping, system chrome, and typography problems."
          "Record reusable lessons with record_experience or `fighorse experience add` after a fix."]
   :must_not ["Do not silently choose a platform, framework, density, or asset format."
              "Do not invent image assets, icons, controls, or typography when fighorse can export or inspect them."
              "Do not write generated files to protected system locations, dependency directories, or hard-to-discover temporary locations unless explicitly requested."
              "Do not bypass fighorse export path validation or local-write mode when writing assets."
              "Do not ignore diagnostics warnings, especially CANVAS/page scope, truncation, missing screenshots, missing tokens, or unknown platform."
              "Do not hard-code lessons from a previous project, brand, screen, or framework into an unrelated design."
              "Do not discard lessons from visual debugging; persist reusable findings through the experience interface."]
   :export_policy (export-dirs)
   :fallback "If any required detail is missing, ask a concise question instead of guessing."})

(defn output-location-guidance []
  {:kind "fighorse.output-location-guidance.v1"
   :fighorse_home (config/fighorse-home)
   :export_policy (export-dirs)
   :examples [{:purpose "temporary Figma slices and debug screenshots"
               :path "./.fighorse/exports"
               :commit "usually no"}
              {:purpose "app-packaged icons or images"
               :path "./assets/fighorse or the app's resource directory"
               :commit "yes, when required by the implementation"}
              {:purpose "cross-project scratch exports"
               :path "~/.fighorse/exports"
               :commit "no"}]})

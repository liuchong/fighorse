(ns fighorse.mcp.policy
  "Shared MCP tool safety policy."
  (:require [fighorse.config :as config]
            [fighorse.mcp.registry :as registry]))

(def local-write-tool-names
  #{"export_images" "export_component" "download_image_fills"})

(defn write-tool? [legacy-write-names name]
  (or (contains? legacy-write-names name)
      (registry/write-tool-name? name)))

(defn local-write-tool? [name]
  (contains? local-write-tool-names name))

(defn violation [legacy-write-names name]
  (cond
    (and (write-tool? legacy-write-names name)
         (not (config/mcp-write-enabled?)))
    (str "Tool " name " is disabled in readonly mode. Set FIGHORSE_MCP_MODE=write to enable Figma write tools.")

    (and (local-write-tool? name)
         (not (config/mcp-local-write-enabled?)))
    (str "Tool " name " writes local files and requires FIGHORSE_MCP_LOCAL_WRITE=allow. Allowed output roots are ./.fighorse/exports, ./assets/fighorse, and ~/.fighorse/exports.")

    :else nil))

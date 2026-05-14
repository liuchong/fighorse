(ns fighorse.config
  "Configuration loading and defaults."
  (:require [clojure.string :as str]))

(def ^:private fs (js/require "fs"))
(def ^:private path (js/require "path"))
(def ^:private os (js/require "os"))

(defn fighorse-home []
  (or (.-FIGHORSE_HOME js/process.env)
      (.join path (.homedir os) ".fighorse")))

(defn config-path []
  (.join path (fighorse-home) "config.json"))

(defn legacy-config-path []
  (.join path (.homedir os) ".config" "fighorse" "config.json"))

(defn- read-json-file [file]
  (if (.existsSync fs file)
    (try
      (js->clj (js/JSON.parse (.readFileSync fs file "utf8"))
               :keywordize-keys true)
      (catch :default _
        {}))
    {}))

(defn- load-file-config []
  (merge (read-json-file (legacy-config-path))
         (read-json-file (config-path))))

(defn save-config!
  "Persist user config under ~/.fighorse/config.json unless FIGHORSE_HOME overrides it."
  [config]
  (let [file (config-path)]
    (.mkdirSync fs (.dirname path file) #js {:recursive true})
    (.writeFileSync fs file (js/JSON.stringify (clj->js config) nil 2))
    config))

(defn clear-config! []
  (let [file (config-path)]
    (when (.existsSync fs file)
      (.unlinkSync fs file))))

(defn load-config
  "Load config from environment variables."
  []
  (let [file-config (load-file-config)]
    {:token (or (.-FIGMA_TOKEN js/process.env)
                (.-FIGMA_API_KEY js/process.env)
                (:token file-config))
     :config-path (config-path)
     :legacy-config-path (legacy-config-path)
     :fighorse-home (fighorse-home)
     :proxy (or (.-HTTPS_PROXY js/process.env)
                (.-https_proxy js/process.env)
                (.-HTTP_PROXY js/process.env)
                (.-http_proxy js/process.env)
                (.-ALL_PROXY js/process.env)
                (.-all_proxy js/process.env))
     :mcp-mode (or (.-FIGHORSE_MCP_MODE js/process.env)
                   (:mcp-mode file-config)
                   "readonly")
     :mcp-local-write (or (.-FIGHORSE_MCP_LOCAL_WRITE js/process.env)
                          (:mcp-local-write file-config)
                          "deny")}))

(defn mcp-write-enabled? []
  (contains? #{"write" "full" "unsafe"} (:mcp-mode (load-config))))

(defn mcp-local-write-enabled? []
  (contains? #{"allow" "true" "1" "yes"} (:mcp-local-write (load-config))))

(defn require-token! [config]
  (when (str/blank? (:token config))
    (println "Error: FIGMA_TOKEN or FIGMA_API_KEY environment variable required")
    (js/process.exit 1)))

(defn setup-proxy!
  "Apply proxy settings to process.env for Bun fetch to pick up.
   Supports HTTP/S proxy. SOCKS5 requires external tool (e.g. proxychains)."
  [proxy-url]
  (when proxy-url
    (when (str/blank? (.-HTTP_PROXY js/process.env))
      (set! (.-HTTP_PROXY js/process.env) proxy-url))
    (when (str/blank? (.-HTTPS_PROXY js/process.env))
      (set! (.-HTTPS_PROXY js/process.env) proxy-url))
    (js/console.error (str "Using proxy: " proxy-url)))
  proxy-url)

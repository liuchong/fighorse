(ns fighorse.discovery-test
  (:require [cljs.test :refer [deftest is testing]]
            [clojure.string]
            [fighorse.discovery :as discovery]))

(def ^:private fs (js/require "fs"))
(def ^:private os (js/require "os"))
(def ^:private path (js/require "path"))

(defn- with-discovery-env [f]
  (let [original-home (.-FIGHORSE_HOME js/process.env)
        original-token (.-FIGMA_TOKEN js/process.env)
        original-api-key (.-FIGMA_API_KEY js/process.env)
        home (.mkdtempSync fs (.join path (.tmpdir os) "fighorse-discovery-"))]
    (set! (.-FIGHORSE_HOME js/process.env) home)
    (js-delete js/process.env "FIGMA_TOKEN")
    (js-delete js/process.env "FIGMA_API_KEY")
    (try
      (f home)
      (finally
        (if (nil? original-home)
          (js-delete js/process.env "FIGHORSE_HOME")
          (set! (.-FIGHORSE_HOME js/process.env) original-home))
        (if (nil? original-token)
          (js-delete js/process.env "FIGMA_TOKEN")
          (set! (.-FIGMA_TOKEN js/process.env) original-token))
        (if (nil? original-api-key)
          (js-delete js/process.env "FIGMA_API_KEY")
          (set! (.-FIGMA_API_KEY js/process.env) original-api-key))))))

(deftest manifest-is-self-describing
  (testing "manifest exposes workflow and contracts"
    (let [m (discovery/manifest)]
      (is (= "fighorse.discovery.v1" (:kind m)))
      (is (= "figma_url" (get-in m [:input_contract :preferred])))
      (is (seq (:recommended_workflow m)))
      (is (= "readonly" (get-in m [:mcp :default_mode])))
      (is (= "http://127.0.0.1:9449/mcp" (get-in m [:mcp :transports :http :url])))
      (is (= "allow" (get-in m [:mcp :transports :stdio :env :FIGHORSE_MCP_LOCAL_WRITE])))
      (is (contains? (set (get-in m [:mcp :local_write :allowed_roots]))
                     "./.fighorse/exports"))
      (is (contains? (set (get-in m [:mcp :self_discovery_tools])) "discover_fighorse"))
      (is (contains? (set (get-in m [:mcp :self_discovery_tools])) "list_experiences"))
      (is (contains? (set (get-in m [:mcp :learning_tools])) "record_experience"))
      (is (contains? (set (get-in m [:mcp :replication_tools])) "get_design_package"))
      (is (contains? (set (get-in m [:mcp :replication_tools])) "export_component"))
      (is (= "fighorse.learned-guidance.v1" (get-in m [:learned_experience :kind])))
      (is (= "fighorse.ai-contract.v1" (get-in m [:ai_contract :kind])))
      (is (= "fighorse.api-coverage.v1" (get-in m [:api_coverage :kind])))
      (is (= 48 (get-in m [:api_coverage :summary :operation_count])))
      (is (seq (get-in m [:official_mcp_comparison :unsupported_by_public_rest])))
      (is (some #(clojure.string/includes? % "manifest")
                (get-in m [:ai_contract :must])))
      (is (some #(clojure.string/includes? % "transferable patterns")
                (get-in m [:ai_contract :must])))
      (is (some #(clojure.string/includes? % "previous project")
                (get-in m [:ai_contract :must_not])))
      (is (some #(clojure.string/includes? % ".fighorse/exports")
                (get-in m [:output_locations :export_policy :rules])))
      (is (seq (:experience_loop m)))
      (is (some #(clojure.string/includes? % "target platform")
                (:quality_rules m)))
      (is (some #(clojure.string/includes? % "record")
                (:quality_rules m)))
      (is (some #(clojure.string/includes? % ".fighorse/exports")
                (:quality_rules m)))
      (is (contains? (set (get-in m [:output_contracts :design_package :contains]))
                     "fidelity_workflow")))))

(deftest mcp-config-defaults-to-http-service
  (testing "config is directly usable by MCP clients"
    (let [cfg (discovery/mcp-config :client "cursor")]
      (is (= "fighorse.mcp-config.v1" (:kind cfg)))
      (is (= "http" (:transport cfg)))
      (is (some #(= "list_experiences" %)
                (:recommended_tool_order cfg)))
      (is (= "http://127.0.0.1:9449/mcp" (get-in cfg [:config :url])))
      (is (= "http://127.0.0.1:9449/mcp"
             (get-in cfg [:examples :cursor :mcpServers :fighorse :url]))))))

(deftest quickstart-guides-first-public-run
  (testing "quickstart parses a selected Figma URL and stays read-only"
    (with-discovery-env
      (fn [_home]
        (let [url "https://www.figma.com/design/abc123/Test?node-id=1-2"
              report (discovery/quickstart :figma-url url)
              markdown (discovery/quickstart->markdown report)]
          (is (= "fighorse.quickstart.v1" (:kind report)))
          (is (= "abc123" (get-in report [:figma_url :file_key])))
          (is (= "1:2" (get-in report [:figma_url :node_id])))
          (is (= false (get-in report [:auth :has_token])))
          (is (some #(= "auth" (:id %)) (:checks report)))
          (is (some #(clojure.string/includes? % "design package")
                    (:next_steps report)))
          (is (clojure.string/includes? markdown "fighorse Quickstart")))))))

(deftest doctor-exposes-actionable-public-diagnostics
  (testing "doctor includes token, service, local-write, and handshake guidance"
    (with-discovery-env
      (fn [_home]
        (let [report (discovery/doctor)]
          (is (some #(= "token" (:id %)) (:checks report)))
          (is (some #(= "mcp_service" (:id %)) (:checks report)))
          (is (some #(= "mcp_repeated_handshake" (:id %)) (:checks report)))
          (is (contains? (:troubleshooting report) :mcp_unexpected_content_type))
          (is (= "http://127.0.0.1:9449/mcp"
                 (get-in report [:mcp_service :endpoint]))))))))

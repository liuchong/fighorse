(ns fighorse.discovery-test
  (:require [cljs.test :refer [deftest is testing]]
            [clojure.string]
            [fighorse.discovery :as discovery]))

(deftest manifest-is-self-describing
  (testing "manifest exposes workflow and contracts"
    (let [m (discovery/manifest)]
      (is (= "fighorse.discovery.v1" (:kind m)))
      (is (= "figma_url" (get-in m [:input_contract :preferred])))
      (is (seq (:recommended_workflow m)))
      (is (= "readonly" (get-in m [:mcp :default_mode])))
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

(deftest mcp-config-defaults-to-stdio
  (testing "config is directly usable by MCP clients"
    (let [cfg (discovery/mcp-config :client "cursor")]
      (is (= "fighorse.mcp-config.v1" (:kind cfg)))
      (is (= "stdio" (:transport cfg)))
      (is (some #(= "list_experiences" %)
                (:recommended_tool_order cfg)))
      (is (= ["mcp" "serve" "--transport" "stdio"] (get-in cfg [:config :args])))
      (is (= "readonly" (get-in cfg [:config :env :FIGHORSE_MCP_MODE])))
      (is (= "allow" (get-in cfg [:config :env :FIGHORSE_MCP_LOCAL_WRITE]))))))

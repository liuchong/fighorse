(ns fighorse.api.coverage-test
  (:require [cljs.test :refer [deftest is testing]]
            [fighorse.api.coverage :as coverage]))

(deftest official-openapi-coverage-registry-is-complete
  (testing "tracks the current vendored OpenAPI operation set"
    (is (= "0.38.0" coverage/openapi-version))
    (is (= 48 (count coverage/official-operations)))
    (is (= 48 (count (distinct (map :operation_id coverage/official-operations)))))
    (is (= 48 (get-in (coverage/coverage-report) [:summary :covered_count])))
    (is (= 11 (get-in (coverage/coverage-report) [:summary :write_count]))))
  (testing "contains known previously missing or drifted operations"
    (let [ids (set (map :operation_id coverage/official-operations))]
      (is (contains? ids "getDeveloperLogs"))
      (is (contains? ids "postVariables"))
      (is (contains? ids "postDevResources"))
      (is (contains? ids "putDevResources"))
      (is (contains? ids "putWebhook"))
      (is (contains? ids "getWebhookRequests"))
      (is (contains? ids "getLibraryAnalyticsStyleActions"))))
  (testing "derives official MCP tool names"
    (is (= "figma_get_file" (coverage/mcp-tool-name "getFile")))
    (is (= "putWebhook" (:operation_id (coverage/operation-for-tool-name "figma_put_webhook"))))))

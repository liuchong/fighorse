(ns fighorse.mcp.tools-test
  (:require [cljs.test :refer [async deftest is testing]]
            [clojure.string :as str]
            [fighorse.export.images :as img-export]
            [fighorse.mcp.tools :as tools]))

(def ^:private fs (js/require "fs"))
(def ^:private os (js/require "os"))
(def ^:private path (js/require "path"))

(defn- result->data [result]
  (js->clj (js/JSON.parse (-> result .-content first .-text))
           :keywordize-keys true))

(deftest list-tools-test
  (testing "returns tools array"
    (let [result (js->clj (tools/list-tools) :keywordize-keys true)
          tools-array (:tools result)]
      (is (> (count tools-array) 0))
      (is (some? (:name (first tools-array))))
      (is (some? (:description (first tools-array)))))))

(deftest tool-definitions-include-key-tools
  (testing "has get_file tool"
    (let [result (js->clj (tools/list-tools) :keywordize-keys true)
          names (map :name (:tools result))]
      (is (some #(= "get_file" %) names))))
  (testing "has get_file_compact tool"
    (let [result (js->clj (tools/list-tools) :keywordize-keys true)
          names (map :name (:tools result))]
      (is (some #(= "get_file_compact" %) names))))
  (testing "has get_me tool"
    (let [result (js->clj (tools/list-tools) :keywordize-keys true)
          names (map :name (:tools result))]
      (is (some #(= "get_me" %) names))))
  (testing "has AI workflow tools"
    (let [result (js->clj (tools/list-tools) :keywordize-keys true)
          names (set (map :name (:tools result)))]
      (is (contains? names "discover_fighorse"))
      (is (contains? names "parse_figma_url"))
      (is (contains? names "get_replicate_workflow"))
      (is (contains? names "get_design_package"))
      (is (contains? names "get_design_context"))
      (is (contains? names "get_node"))
      (is (contains? names "get_tokens"))
      (is (contains? names "get_screenshot"))
      (is (contains? names "export_images"))
      (is (contains? names "export_component"))
      (is (contains? names "download_image_fills"))
      (is (contains? names "get_experience_schema"))
      (is (contains? names "list_experiences"))
      (is (contains? names "record_experience"))))
  (testing "has extended REST coverage tools"
    (let [result (js->clj (tools/list-tools) :keywordize-keys true)
          names (set (map :name (:tools result)))]
      (is (contains? names "get_oembed"))
      (is (contains? names "get_payments"))
      (is (contains? names "get_activity_logs"))
      (is (contains? names "get_library_analytics_component_usages"))))
  (testing "write tools are hidden by default"
    (let [result (js->clj (tools/list-tools) :keywordize-keys true)
          names (set (map :name (:tools result)))]
      (is (not (contains? names "post_comment")))
      (is (not (contains? names "create_webhook"))))))

(deftest get-design-package-schema-includes-target-controls
  (testing "AI clients can discover platform and asset format inputs"
    (let [result (js->clj (tools/list-tools) :keywordize-keys true)
          tool (first (filter #(= "get_design_package" (:name %)) (:tools result)))
          props (get-in tool [:inputSchema :properties])]
      (is (contains? props :platform))
      (is (contains? props :asset_format)))))

(deftest handle-tool-unknown-test
  (testing "unknown tool returns error"
    (async done
      (let [p (tools/handle-tool "nonexistent_tool" {})
            result (atom nil)]
      (-> p
          (.then (fn [r] (reset! result r)))
          (.then (fn []
                   (is (= true (.-isError ^js @result)))
                   (is (str/includes?
                        (-> @result .-content first .-text)
                        "Unknown tool"))))
          (.finally done))))))

(deftest self-discovery-tools-do-not-require-figma-token
  (testing "discover_fighorse returns manifest"
    (async done
      (-> (tools/handle-tool "discover_fighorse" {})
          (.then (fn [result]
                   (let [text (-> result .-content first .-text)
                         data (js->clj (js/JSON.parse text) :keywordize-keys true)]
                     (is (= "fighorse.discovery.v1" (:kind data)))
                     (is (some #(= "list_experiences" (:tool %))
                               (:recommended_workflow data)))
                     (is (some #(= "get_design_package" (:tool %))
                               (:recommended_workflow data))))))
          (.finally done)))))

(deftest experience-tools-record-and-list-local-lessons
  (testing "MCP can persist and return local learned experience without Figma writes"
    (async done
      (let [original-path (.-FIGHORSE_EXPERIENCE_PATH js/process.env)
            store (.join path
                         (.mkdtempSync fs (.join path (.tmpdir os) "fighorse-mcp-experience-"))
                         "experience.jsonl")]
        (set! (.-FIGHORSE_EXPERIENCE_PATH js/process.env) store)
        (-> (tools/handle-tool "record_experience"
                               {:summary "Small card typography drifted"
                                :lesson "Read compact card textStyle instead of reusing full-screen typography."
                                :category "typography"
                                :platform "android-compose"
                                :asset_format "png"
                                :tags ["compose" "font"]})
            (.then (fn [result]
                     (let [data (result->data result)]
                       (is (= "fighorse.experience-write.v1" (:kind data)))
                       (is (= store (:store_path data))))))
            (.then (fn []
                     (tools/handle-tool "list_experiences"
                                        {:platform "android-compose"
                                         :asset_format "png"})))
            (.then (fn [result]
                     (let [data (result->data result)]
                       (is (= "fighorse.learned-guidance.v1" (:kind data)))
                       (is (= 1 (get-in data [:summary :returned_count])))
                       (is (= "Small card typography drifted"
                              (get-in data [:summary :records 0 :summary]))))))
            (.then (fn []
                     (tools/handle-tool "get_experience_schema" {})))
            (.then (fn [result]
                     (is (= "fighorse.experience-schema.v1"
                            (:kind (result->data result))))))
            (.catch (fn [err]
                      (is false (str err))))
            (.finally (fn []
                        (if (nil? original-path)
                          (js-delete js/process.env "FIGHORSE_EXPERIENCE_PATH")
                          (set! (.-FIGHORSE_EXPERIENCE_PATH js/process.env) original-path))
                        (done))))))))

(deftest parse-figma-url-tool-test
  (testing "parse_figma_url normalizes pasted URL"
    (async done
      (-> (tools/handle-tool "parse_figma_url"
                             {:figma_url "https://www.figma.com/design/abc/Mock?node-id=1-2"})
          (.then (fn [result]
                   (let [text (-> result .-content first .-text)
                         data (js->clj (js/JSON.parse text) :keywordize-keys true)]
                     (is (= "abc" (:file_key data)))
                     (is (= "1:2" (:node_id data))))))
          (.finally done)))))

(deftest call-tool-blocks-write-tools-in-readonly-mode
  (testing "write tools fail closed through MCP call-tool"
    (async done
      (let [request #js {:params #js {:name "post_comment"
                                      :arguments #js {:file_key "abc"
                                                      :message "hello"}}}]
        (-> (tools/call-tool request)
            (.then (fn [result]
                     (is (= true (.-isError ^js result)))
                     (is (str/includes? (-> result .-content first .-text)
                                        "readonly mode"))))
            (.finally done))))))

(deftest call-tool-requires-local-write-for-export-tools
  (testing "local filesystem export is controlled separately from Figma write mode"
    (async done
      (let [original-local-write (.-FIGHORSE_MCP_LOCAL_WRITE js/process.env)
            request #js {:params #js {:name "export_images"
                                      :arguments #js {:file_key "abc"
                                                      :node_ids "1:2"
                                                      :dest_dir "./.fighorse/exports"}}}]
        (js-delete js/process.env "FIGHORSE_MCP_LOCAL_WRITE")
        (-> (tools/call-tool request)
            (.then (fn [result]
                     (is (= true (.-isError ^js result)))
                     (is (str/includes? (-> result .-content first .-text)
                                        "FIGHORSE_MCP_LOCAL_WRITE=allow"))))
            (.then (fn []
                     (set! (.-FIGHORSE_MCP_LOCAL_WRITE js/process.env) "allow")
                     (with-redefs [img-export/export-images
                                   (fn [& _args]
                                     (js/Promise.resolve {"1:2" ".fighorse/exports/1_2.png"}))]
                       (tools/call-tool request))))
            (.then (fn [result]
                     (is (not (.-isError ^js result)))
                     (is (= ".fighorse/exports/1_2.png"
                            (get (result->data result) (keyword "1:2"))))))
            (.finally (fn []
                        (if (nil? original-local-write)
                          (js-delete js/process.env "FIGHORSE_MCP_LOCAL_WRITE")
                          (set! (.-FIGHORSE_MCP_LOCAL_WRITE js/process.env) original-local-write))
                        (done))))))))

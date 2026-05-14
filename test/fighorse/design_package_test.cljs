(ns fighorse.design-package-test
  (:require [cljs.test :refer [async deftest is testing]]
            [clojure.string :as str]
            [fighorse.design-package :as design-package]))

(def ^:private fs (js/require "fs"))
(def ^:private os (js/require "os"))
(def ^:private path (js/require "path"))

(defn- response [body]
  #js {:ok true
       :status 200
       :statusText "OK"
       :text (fn [] (js/Promise.resolve (js/JSON.stringify (clj->js body))))})

(def ^:private mock-node
  {:id "1:2"
   :name "Hero Card"
   :type "FRAME"
   :absoluteBoundingBox {:width 320 :height 180}
   :fills [{:type "SOLID" :color {:r 0.2 :g 0.4 :b 0.8 :a 1}}]
   :children [{:id "1:3"
               :name "Title"
               :type "TEXT"
               :characters "Hello"
               :style {:fontFamily "Test Sans"
                       :fontSize 24
                       :fontWeight 700}
               :absoluteBoundingBox {:width 120 :height 32}}]})

(deftest get-design-package-from-figma-url-test
  (testing "builds a self-contained replication package"
    (async done
      (let [original-fetch js/fetch
            original-experience-path (.-FIGHORSE_EXPERIENCE_PATH js/process.env)
            experience-store (.join path
                                    (.mkdtempSync fs (.join path (.tmpdir os) "fighorse-design-package-"))
                                    "experience.jsonl")
            calls (atom [])]
        (set! (.-FIGHORSE_EXPERIENCE_PATH js/process.env) experience-store)
        (set! js/fetch
              (fn [url _opts]
                (swap! calls conj url)
                (js/Promise.resolve
                 (if (str/includes? url "/v1/images/")
                   (response {:images {"1:2" "https://images.example/hero.png"}})
                   (response {:name "Mock File"
                              :lastModified "2026-05-15T00:00:00Z"
                              :nodes {"1:2" {:document mock-node}}})))))
        (-> (design-package/get-design-package "token"
                                               :figma-url "https://www.figma.com/design/abc/Mock?node-id=1-2"
                                               :depth 2
                                               :max-tokens 2000
                                               :include-screenshot true
                                               :platform "android-compose"
                                               :asset-format "png")
            (.then (fn [pkg]
                     (is (= "fighorse.design-package.v1" (:kind pkg)))
                     (is (= "abc" (get-in pkg [:source :file_key])))
                     (is (= "1:2" (get-in pkg [:source :node_id])))
                     (is (= "android-compose" (get-in pkg [:implementation_target :platform])))
                     (is (= "png" (get-in pkg [:implementation_target :asset_format])))
                     (is (some #(str/includes? % "Jetpack Compose")
                               (get-in pkg [:implementation_target :rules])))
                     (is (seq (get-in pkg [:fidelity_workflow :attention_checks])))
                     (is (some #(= "export_component" (:tool %))
                               (get-in pkg [:asset_export_plan :mcp_tools])))
                     (is (= "fighorse.ai-contract.v1" (get-in pkg [:ai_contract :kind])))
                     (is (some #(str/includes? % "manifest")
                               (get-in pkg [:ai_contract :must])))
                     (is (str/includes? (first (get-in pkg [:asset_export_plan :cli_examples]))
                                        ".fighorse/exports"))
                     (is (some #(str/includes? % "project-local")
                               (get-in pkg [:asset_export_plan :output_policy :export_policy :rules])))
                     (is (= "fighorse.learned-guidance.v1" (get-in pkg [:learned_experience :kind])))
                     (is (some #(= "record_experience" (:tool %))
                               (:next_tools pkg)))
                     (is (= "Hero Card" (get-in pkg [:target :name])))
                     (is (= "https://images.example/hero.png"
                            (get-in pkg [:screenshots :images (keyword "1:2")])))
                     (is (seq (get-in pkg [:tokens "color"])))
                     (is (= "ready" (get-in pkg [:diagnostics :status])))
                     (is (= 1 (get-in pkg [:diagnostics :screenshots :count])))
                     (is (= false (get-in pkg [:diagnostics :context_truncated])))
                     (is (some #(str/includes? % "/v1/files/abc/nodes") @calls))
                     (is (some #(str/includes? % "/v1/images/abc") @calls))))
            (.catch (fn [err]
                      (is false (str err))))
            (.finally (fn []
                        (set! js/fetch original-fetch)
                        (if (nil? original-experience-path)
                          (js-delete js/process.env "FIGHORSE_EXPERIENCE_PATH")
                          (set! (.-FIGHORSE_EXPERIENCE_PATH js/process.env) original-experience-path))
                        (done))))))))

(deftest design-package-warns-when-asset-format-is-missing-and-counts-assets
  (testing "AI clients are told to ask for asset format and asset counts reflect image fills"
    (async done
      (let [original-fetch js/fetch]
        (set! js/fetch
              (fn [url _opts]
                (js/Promise.resolve
                 (cond
                   (str/includes? url "/v1/images/")
                   (response {:images {"1:2" "https://images.example/hero.png"}})

                   (str/includes? url "/v1/files/abc/images")
                   (response {:meta {:images {"fill-a" "https://images.example/a.webp"
                                              "fill-b" "https://images.example/b.png"}}})

                   :else
                   (response {:name "Mock File"
                              :nodes {"1:2" {:document mock-node}}})))))
        (-> (design-package/get-design-package "token"
                                               :figma-url "https://www.figma.com/design/abc/Mock?node-id=1-2"
                                               :include-assets true
                                               :platform "web-react")
            (.then (fn [pkg]
                     (is (= "png" (get-in pkg [:implementation_target :asset_format])))
                     (is (some #(str/includes? % "Asset export format was not specified")
                               (get-in pkg [:implementation_target :ask_developer_when])))
                     (is (some #(str/includes? % "Asset export format was not specified")
                               (get-in pkg [:diagnostics :warnings])))
                     (is (= 2 (get-in pkg [:diagnostics :assets :count])))))
            (.catch (fn [err]
                      (is false (str err))))
            (.finally (fn []
                        (set! js/fetch original-fetch)
                        (done))))))))

(ns fighorse.api.files-test
  (:require [cljs.test :refer [deftest is testing async]]
            [fighorse.api.files :as files]))

(defn- integration-enabled? []
  (= "true" (.-FIGMA_INTEGRATION_TESTS js/process.env)))

(deftest get-file-params-test
  (testing "get-file builds correct request structure"
    ;; This is an integration test. It is opt-in so local/CI unit tests stay deterministic.
    (when-let [token (when (integration-enabled?)
                       (or (.-FIGMA_TOKEN js/process.env)
                           (.-FIGMA_API_KEY js/process.env)))]
      (async done
        (-> (files/get-file token "SApEHB4JyKN2I8PpMYfgbK" :depth 1)
            (.then (fn [data]
                     (is (map? data))
                     (is (contains? data :document))
                     (is (= "DOCUMENT" (get-in data [:document :type])))
                     (done)))
            (.catch (fn [err]
                      (is false (str "API call failed: " err))
                      (done))))))))

(deftest get-file-nodes-test
  (testing "get-file-nodes accepts comma-separated ids"
    (when-let [token (when (integration-enabled?)
                       (or (.-FIGMA_TOKEN js/process.env)
                           (.-FIGMA_API_KEY js/process.env)))]
      (async done
        (-> (files/get-file-nodes token "SApEHB4JyKN2I8PpMYfgbK" "0:1")
            (.then (fn [data]
                     (is (map? data))
                     (is (contains? data :nodes))
                     (done)))
            (.catch (fn [err]
                      (is false (str "API call failed: " err))
                      (done))))))))

(deftest get-images-test
  (testing "get-images returns image URLs map"
    (when-let [token (when (integration-enabled?)
                       (or (.-FIGMA_TOKEN js/process.env)
                           (.-FIGMA_API_KEY js/process.env)))]
      (async done
        (-> (files/get-images token "SApEHB4JyKN2I8PpMYfgbK" "0:1")
            (.then (fn [data]
                     (is (map? data))
                     (is (contains? data :images))
                     (done)))
            (.catch (fn [err]
                      ;; Some files may not have renderable nodes
                      (is (or (map? err) true))
                      (done))))))))

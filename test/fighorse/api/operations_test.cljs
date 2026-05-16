(ns fighorse.api.operations-test
  (:require [cljs.test :refer [async deftest is testing]]
            [fighorse.api.developer-logs :as developer-logs-api]
            [fighorse.api.dev-resources :as dev-resources-api]
            [fighorse.api.operations :as operations]
            [fighorse.api.variables :as variables-api]
            [fighorse.api.webhooks :as webhooks-api]))

(deftest developer-logs-operation-uses-post-style-body-filters
  (testing "developer logs dispatches body filters to the API wrapper"
    (async done
      (with-redefs [developer-logs-api/get-developer-logs
                    (fn [token & opts]
                      (js/Promise.resolve {:token token :opts opts}))]
        (-> (operations/call-operation "token" "getDeveloperLogs" {:limit 10} {:date_range "last_7d"})
            (.then (fn [result]
                     (is (= "token" (:token result)))
                     (is (= "last_7d" (get (apply hash-map (:opts result)) :date-range)))))
            (.finally done))))))

(deftest variables-operation-uses-bulk-variables-endpoint
  (testing "variables write uses the bulk /variables operation"
    (async done
      (with-redefs [variables-api/post-variables
                    (fn [token file-key body]
                      (js/Promise.resolve {:token token :file-key file-key :body body}))]
        (let [body {:variables [{:action "CREATE" :id "tmp"}]}]
          (-> (operations/call-operation "token" "postVariables" {:file_key "abc"} body)
              (.then (fn [result]
                       (is (= "abc" (:file-key result)))
                       (is (= body (:body result)))))
              (.finally done)))))))

(deftest dev-resource-operations-use-official-bulk-endpoints
  (testing "dev resources create/update use official bulk endpoints"
    (async done
      (with-redefs [dev-resources-api/post-dev-resources
                    (fn [_token resources]
                      (js/Promise.resolve {:created resources}))
                    dev-resources-api/put-dev-resources
                    (fn [_token resources]
                      (js/Promise.resolve {:updated resources}))]
        (let [resources [{:name "Docs" :url "https://example.com" :file_key "abc" :node_id "1:2"}]]
          (-> (operations/call-operation "token" "postDevResources" {} {:dev_resources resources})
              (.then (fn [result]
                       (is (= resources (:created result)))))
              (.then (fn []
                       (operations/call-operation "token" "putDevResources" {} {:dev_resources resources})))
              (.then (fn [result]
                       (is (= resources (:updated result)))))
              (.finally done)))))))

(deftest webhook-operation-exposes-put
  (testing "webhook update is exposed as PUT"
    (async done
      (with-redefs [webhooks-api/update-webhook
                    (fn [_token webhook-id body]
                      (js/Promise.resolve {:webhook-id webhook-id :body body}))]
        (-> (operations/call-operation "token" "putWebhook" {:webhook_id "wh1"} {:status "PAUSED"})
            (.then (fn [result]
                     (is (= "wh1" (:webhook-id result)))
                     (is (= {:status "PAUSED"} (:body result)))))
            (.finally done))))))

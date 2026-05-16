(ns fighorse.utils.http-test
  (:require [cljs.test :refer [async deftest is testing]]
            [fighorse.utils.http :as http]))

(defn- response [ok status body]
  #js {:ok ok
       :status status
       :statusText (if ok "OK" "Bad Request")
       :text (fn [] (js/Promise.resolve body))})

(deftest delete-empty-response-test
  (testing "204 responses resolve to an empty map"
    (async done
      (let [original-fetch js/fetch]
        (set! js/fetch (fn [_url opts]
                         (is (= "DELETE" (.-method opts)))
                         (js/Promise.resolve (response true 204 ""))))
        (-> (http/delete "/v1/test" "token")
            (.then (fn [body]
                     (is (= {} body))))
            (.catch (fn [err]
                      (is false (str err))))
            (.finally (fn []
                        (set! js/fetch original-fetch)
                        (done))))))))

(deftest post-json-options-test
  (testing "POST requests set method, auth header, and JSON body"
    (async done
      (let [original-fetch js/fetch
            captured (atom nil)]
        (set! js/fetch (fn [url opts]
                         (reset! captured {:url url :opts opts})
                         (js/Promise.resolve (response true 200 "{\"ok\":true}"))))
        (-> (http/post "/v1/test" "token" :body {:a 1})
            (.then (fn [body]
                     (is (= {:ok true} body))
                     (is (= "https://api.figma.com/v1/test" (:url @captured)))
                     (is (= "POST" (.-method (:opts @captured))))
                     (is (= "{\"a\":1}" (.-body (:opts @captured))))
                     (is (= "token" (aget (.-headers (:opts @captured)) "X-Figma-Token")))))
            (.catch (fn [err]
                      (is false (str err))))
            (.finally (fn []
                        (set! js/fetch original-fetch)
                        (done))))))))

(deftest put-json-options-test
  (testing "PUT requests are supported for current Figma REST operations"
    (async done
      (let [original-fetch js/fetch
            captured (atom nil)]
        (set! js/fetch (fn [url opts]
                         (reset! captured {:url url :opts opts})
                         (js/Promise.resolve (response true 200 "{\"ok\":true}"))))
        (-> (http/put "/v1/test" "token" :body {:a 1})
            (.then (fn [body]
                     (is (= {:ok true} body))
                     (is (= "PUT" (.-method (:opts @captured))))
                     (is (= "{\"a\":1}" (.-body (:opts @captured))))))
            (.catch (fn [err]
                      (is false (str err))))
            (.finally (fn []
                        (set! js/fetch original-fetch)
                        (done))))))))

(deftest error-response-test
  (testing "non-2xx responses reject with status and parsed body"
    (async done
      (let [original-fetch js/fetch]
        (set! js/fetch (fn [_url _opts]
                         (js/Promise.resolve (response false 400 "{\"message\":\"bad\"}"))))
        (-> (http/get "/v1/test" "token")
            (.then (fn [_]
                     (is false "request should reject")))
            (.catch (fn [err]
                      (is (= 400 (:status (ex-data err))))
                      (is (= "bad" (get-in (ex-data err) [:body :message])))))
            (.finally (fn []
                        (set! js/fetch original-fetch)
                        (done))))))))

(deftest path-segment-encodes-url-path-values
  (testing "path values cannot inject extra URL segments"
    (is (= "a%2Fb%3Fc%3Dd" (http/path-segment "a/b?c=d")))))

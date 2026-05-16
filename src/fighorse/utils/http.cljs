(ns fighorse.utils.http
  "Unified HTTP client for Figma REST API.
   All requests return Promise<clj-map>.
   Supports common REST verbs with automatic JSON handling."
  (:refer-clojure :exclude [get])
  (:require [clojure.string :as str]
            [fighorse.utils.url :as url]))

(def ^:private base-url "https://api.figma.com")

(defn- auth-headers [token]
  (cond-> {"Content-Type" "application/json"}
    (not (str/blank? token)) (assoc "X-Figma-Token" token)))

(defn- parse-body [text]
  (if (str/blank? text)
    {}
    (try
      (js->clj (js/JSON.parse text) :keywordize-keys true)
      (catch :default _
        {:raw text}))))

(defn- parse-json-response [response]
  (-> response
      (.text)
      (.then parse-body)))

(defn- handle-error [response]
  (if (.-ok response)
    (js/Promise.resolve response)
    (-> (parse-json-response response)
        (.then (fn [body]
                 (js/Promise.reject
                  (ex-info (str "Figma API error: " (.-status response) " " (.-statusText response))
                           {:status (.-status response)
                            :body body})))))))

(defn request
  "Make an HTTP request to the Figma API.
   - method: :get, :post, :put, :patch, :delete
   - path: API path (e.g. '/v1/files/abc')
   - token: Figma Personal Access Token
   - opts: optional map with :params (query params) and :body (request body)"
  [method path token & {:keys [params body]}]
  (let [url (url/build-url (str base-url path) params)
        opts (cond-> {:headers (auth-headers token)}
               (= method :post) (assoc :method "POST")
               (= method :put) (assoc :method "PUT")
               (= method :patch) (assoc :method "PATCH")
               (= method :delete) (assoc :method "DELETE")
               body (assoc :body (js/JSON.stringify (clj->js body))))]
    (-> (js/fetch url (clj->js opts))
        (.then handle-error)
        (.then parse-json-response))))

(defn path-segment [value]
  (js/encodeURIComponent (str value)))

(defn get [path token & {:keys [params]}]
  (request :get path token :params params))

(defn post [path token & {:keys [params body]}]
  (request :post path token :params params :body body))

(defn put [path token & {:keys [params body]}]
  (request :put path token :params params :body body))

(defn patch [path token & {:keys [params body]}]
  (request :patch path token :params params :body body))

(defn delete [path token & {:keys [params]}]
  (request :delete path token :params params))

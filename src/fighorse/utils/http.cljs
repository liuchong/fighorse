(ns fighorse.utils.http
  "Unified HTTP client for Figma REST API.
   All requests return Promise<clj-map>.
   Supports common REST verbs with automatic JSON handling."
  (:refer-clojure :exclude [get])
  (:require [clojure.string :as str]
            [fighorse.utils.url :as url]))

(def ^:private base-url "https://api.figma.com")
(def ^:private default-timeout-ms 120000)
(defonce ^:private active-controllers (atom #{}))

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

(defn request-timeout-ms []
  (let [raw (.-FIGHORSE_HTTP_TIMEOUT_MS js/process.env)
        parsed (when raw (js/parseInt raw 10))]
    (if (and parsed (not (js/isNaN parsed)) (pos? parsed))
      parsed
      default-timeout-ms)))

(defn abort-active-requests!
  "Abort in-flight Figma HTTP requests. Used by one-shot CLI signal handling."
  []
  (doseq [controller @active-controllers]
    (try
      (.abort controller)
      (catch :default _ nil)))
  (reset! active-controllers #{})
  nil)

(defn fetch-with-timeout
  "Fetch with the shared timeout/abort registry used by CLI and asset downloads."
  [url opts]
  (let [timeout-ms (request-timeout-ms)
        AbortController (.-AbortController js/globalThis)
        controller (when AbortController (new AbortController))
        timeout-id (when controller
                     (js/setTimeout (fn [] (.abort controller)) timeout-ms))
        opts (cond-> (or opts {})
               controller (assoc :signal (.-signal controller)))]
    (when controller
      (swap! active-controllers conj controller))
    (-> (js/fetch url (clj->js opts))
        (.catch (fn [err]
                  (if (= "AbortError" (.-name err))
                    (js/Promise.reject (js/Error. (str "Figma API request timed out after " timeout-ms "ms")))
                    (js/Promise.reject err))))
        (.finally (fn []
                    (when controller
                      (swap! active-controllers disj controller))
                    (when timeout-id
                      (js/clearTimeout timeout-id)))))))

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
    (-> (fetch-with-timeout url opts)
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

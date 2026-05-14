(ns fighorse.api
  (:require [clojure.string :as str]))

(def ^:private base-url "https://api.figma.com/v1")

(defn- headers [token]
  #js {"X-Figma-Token" token
       "Content-Type" "application/json"})

(defn fetch-file
  "Fetch raw Figma file JSON. Returns a Promise<clj-map>."
  [file-key token & {:keys [depth]}]
  (let [url (str base-url "/files/" file-key
                 (when depth (str "?depth=" depth)))]
    (-> (js/fetch url #js {:headers (headers token)})
        (.then #(.json %))
        (.then #(js->clj % :keywordize-keys true)))))

(defn fetch-node
  "Fetch specific node from Figma file. Returns a Promise<clj-map>."
  [file-key node-id token & {:keys [depth]}]
  (let [url (str base-url "/files/" file-key "/nodes?ids=" node-id
                 (when depth (str "&depth=" depth)))]
    (-> (js/fetch url #js {:headers (headers token)})
        (.then #(.json %))
        (.then #(js->clj % :keywordize-keys true)))))

(defn fetch-image-urls
  "Get render URLs for nodes. Returns a Promise<clj-map of id->url>."
  [file-key node-ids token & {:keys [format scale]
                              :or {format "png" scale 2}}]
  (let [ids-str (str/join "," node-ids)
        url (str base-url "/images/" file-key
                 "?ids=" ids-str
                 "&format=" format
                 "&scale=" scale)]
    (-> (js/fetch url #js {:headers (headers token)})
        (.then #(.json %))
        (.then #(js->clj % :keywordize-keys true))
        (.then #(get-in % [:images])))))

(ns fighorse.utils.url
  "URL and query string utilities."
  (:require [clojure.string :as str]))

(defn- encode-param [[k v]]
  (when (some? v)
    (str (name k) "=" (js/encodeURIComponent (str v)))))

(defn build-query
  "Build URL query string from a map. Nil values are omitted."
  [params]
  (let [parts (keep encode-param params)]
    (when (seq parts)
      (str "?" (str/join "&" parts)))))

(defn build-url
  "Build full URL with optional query params."
  [base-path params]
  (str base-path (build-query params)))

(defn normalize-node-id
  "Convert Figma URL node-id form (1-2) to API form (1:2)."
  [node-id]
  (when (seq node-id)
    (str/replace node-id #"-" ":")))

(defn- url? [s]
  (boolean (re-find #"^https?://" s)))

(defn- path-segments [^js parsed]
  (->> (str/split (.-pathname parsed) #"/")
       (remove str/blank?)
       vec))

(defn- file-key-from-segments [segments]
  (some (fn [[kind file-key]]
          (when (#{"design" "file" "proto" "board"} kind)
            file-key))
        (partition 2 1 segments)))

(defn parse-figma-url
  "Parse a Figma URL or raw file key.

   Returns a map with :valid, :file_key, and optional :node_id.
   Figma URL node IDs use dashes in query params; REST APIs require colons."
  [input]
  (let [raw (str/trim (or input ""))]
    (cond
      (str/blank? raw)
      {:valid false
       :input raw
       :error "Empty Figma URL or file key"}

      (not (url? raw))
      {:valid true
       :input raw
       :file_key raw
       :kind "file_key"}

      :else
      (try
        (let [parsed (js/URL. raw)
              embedded (.get (.-searchParams parsed) "url")]
          (if (seq embedded)
            (assoc (parse-figma-url embedded) :input raw :embedded_url embedded)
            (let [segments (path-segments parsed)
                  file-key (file-key-from-segments segments)
                  raw-node-id (.get (.-searchParams parsed) "node-id")
                  kind (first segments)]
              (if file-key
                (cond-> {:valid true
                         :input raw
                         :kind kind
                         :file_key file-key}
                  raw-node-id (assoc :raw_node_id raw-node-id
                                     :node_id (normalize-node-id raw-node-id)))
                {:valid false
                 :input raw
                 :error "Could not find Figma file key in URL"}))))
        (catch :default err
          {:valid false
           :input raw
           :error (or (.-message err) (str err))})))))

(ns fighorse.export.images
  "Image export — render and download images from Figma files.
   Uses Figma /v1/images API to get URLs, then downloads them."
  (:require [clojure.string :as str]
            [fighorse.api.files :as files-api]))

(def ^:private fs (js/require "fs"))
(def ^:private path (js/require "path"))
(def ^:private os (js/require "os"))
(def ^:private default-export-dir "./.fighorse/exports")

(defn- normalize-format [format]
  (case (str/lower-case (or (str format) "png"))
    "svg" "svg"
    "pdf" "pdf"
    "jpg" "jpg"
    "jpeg" "jpg"
    "png"))

(defn- format-extension [format]
  (case (normalize-format format)
    "svg" ".svg"
    "pdf" ".pdf"
    "jpg" ".jpg"
    ".png"))

(defn- content-type-extension [content-type]
  (let [content-type (str/lower-case (or content-type ""))]
    (cond
      (str/includes? content-type "png") ".png"
      (str/includes? content-type "jpeg") ".jpg"
      (str/includes? content-type "jpg") ".jpg"
      (str/includes? content-type "svg") ".svg"
      (str/includes? content-type "webp") ".webp"
      (str/includes? content-type "gif") ".gif"
      (str/includes? content-type "pdf") ".pdf"
      :else "")))

(defn- has-extension? [dest-path]
  (boolean (re-find #"\.[A-Za-z0-9]+$" dest-path)))

(defn- ensure-extension [dest-path content-type fallback-ext]
  (if (has-extension? dest-path)
    dest-path
    (let [detected (content-type-extension content-type)]
      (str dest-path (if (str/blank? detected) fallback-ext detected)))))

(defn- fetch-image
  "Download an image from URL to dest-path using Bun. Resolves to the written path."
  [url dest-path & {:keys [fallback-ext] :or {fallback-ext ""}}]
  (-> (js/fetch url)
      (.then (fn [response]
               (when-not (.-ok response)
                 (throw (js/Error. (str "Failed to download image: HTTP " (.-status response)))))
               (.blob response)))
      (.then (fn [blob]
               (let [final-path (ensure-extension dest-path (.-type blob) fallback-ext)]
                 (-> (js/Bun.write final-path blob)
                     (.then (fn [_] final-path))))))))

(defn- safe-name [s]
  (let [clean (-> (str s)
                  (str/replace #"[^A-Za-z0-9._-]+" "_")
                  (str/replace #"^_+" ""))]
    (if (str/blank? clean) "asset" clean)))

(defn- allowed-export-roots []
  (let [cwd (.cwd js/process)
        home (.homedir os)]
    [(.resolve path cwd ".fighorse" "exports")
     (.resolve path cwd "assets" "fighorse")
     (.resolve path home ".fighorse" "exports")]))

(defn- child-path? [root target]
  (let [root (.resolve path root)
        target (.resolve path target)
        sep (.-sep path)]
    (or (= root target)
        (str/starts-with? target (str root sep)))))

(defn- existing-ancestor [target]
  (loop [dir (.resolve path target)]
    (let [parent (.dirname path dir)]
      (cond
        (.existsSync fs dir) dir
        (= parent dir) dir
        :else (recur parent)))))

(defn- canonical-path [target]
  (let [resolved (.resolve path target)
        ancestor (existing-ancestor resolved)
        relative (.relative path ancestor resolved)]
    (.resolve path (.realpathSync fs ancestor) relative)))

(defn- first-allowed-root [dest-dir]
  (let [target (canonical-path dest-dir)]
    (some (fn [root]
            (let [canonical-root (canonical-path root)]
              (when (child-path? canonical-root target)
                root)))
          (allowed-export-roots))))

(defn- safe-export-dir! [dest-dir]
  (let [dest-dir (or dest-dir default-export-dir)
        resolved (.resolve path dest-dir)
        root (first-allowed-root resolved)]
    (when-not root
      (throw (js/Error.
              (str "Export directory is outside allowed roots: " dest-dir
                   ". Use ./.fighorse/exports, ./assets/fighorse, or ~/.fighorse/exports."))))
    (.mkdirSync fs resolved #js {:recursive true})
    (let [real-root (.realpathSync fs root)
          real-dest (.realpathSync fs resolved)]
      (when-not (child-path? real-root real-dest)
        (throw (js/Error. (str "Export directory escapes allowed root: " dest-dir))))
      real-dest)))

(defn- write-manifest! [dir kind entries]
  (.writeFileSync fs
                  (.join path dir "manifest.json")
                  (js/JSON.stringify
                    (clj->js {:kind kind
                              :generated_by "fighorse"
                              :entries entries})
                    nil
                    2)))

(defn export-images
  "Render and download images for specified nodes.
   Returns a promise that resolves to a map of node-id -> downloaded-path."
  [token file-key node-ids & {:keys [format scale dest-dir manifest prefix]
                              :or {format "png" scale 2}}]
  (let [format (normalize-format format)]
    (-> (files-api/get-images token file-key node-ids
                              :format format :scale scale)
      (.then (fn [result]
               (let [images (:images result)
                     dir (safe-export-dir! dest-dir)
                     ext (format-extension format)
                     filename-prefix (if (str/blank? prefix) "" (safe-name prefix))
                     downloads (into [] (for [[node-id image-url] images
                                              :when image-url]
                                          (let [node-id-str (name node-id)
                                                target-path (.join path dir (str filename-prefix (safe-name node-id-str) ext))]
                                            (-> (fetch-image image-url target-path :fallback-ext ext)
                                                (.then (fn [written-path] [node-id-str written-path image-url]))))))]
                 (-> (js/Promise.all (clj->js downloads))
                     (.then (fn [rows-js]
                              (let [rows (into [] (array-seq rows-js))
                                    entries (mapv (fn [row]
                                                    {:node_id (nth row 0)
                                                     :path (nth row 1)
                                                     :format format
                                                     :scale scale
                                                     :source_url (nth row 2)})
                                                  rows)]
                                (when manifest
                                  (write-manifest! dir "fighorse.image_export" entries))
                                (into {} (map (fn [row] [(nth row 0) (nth row 1)]) rows))))))))))))

(defn download-image-fills
  "Download all image fills in a file. Returns image-ref -> downloaded path."
  [token file-key & {:keys [dest-dir manifest prefix]
                     :or {dest-dir default-export-dir}}]
  (-> (files-api/get-image-fills token file-key)
      (.then (fn [result]
               (let [images (or (get-in result [:meta :images])
                                (:images result)
                                {})
                     filename-prefix (or prefix "")
                     dest-dir (safe-export-dir! dest-dir)
                     filename-prefix (if (str/blank? filename-prefix) "" (safe-name filename-prefix))
                     downloads (into [] (for [[image-ref image-url] images
                                              :when image-url]
                                          (let [image-ref-str (name image-ref)
                                                target-path (.join path dest-dir (str filename-prefix (safe-name image-ref-str)))]
                                            (-> (fetch-image image-url target-path)
                                                (.then (fn [written-path] [image-ref-str written-path image-url]))))))]
                 (-> (js/Promise.all (clj->js downloads))
                     (.then (fn [rows-js]
                              (let [rows (into [] (array-seq rows-js))
                                    entries (mapv (fn [row]
                                                    {:image_ref (nth row 0)
                                                     :path (nth row 1)
                                                     :source_url (nth row 2)})
                                                  rows)]
                                (when manifest
                                  (write-manifest! dest-dir "fighorse.asset_download" entries))
                                (into {} (map (fn [row] [(nth row 0) (nth row 1)]) rows)))))))))))

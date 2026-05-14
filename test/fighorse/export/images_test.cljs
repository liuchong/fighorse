(ns fighorse.export.images-test
  (:require [cljs.test :refer [async deftest is testing]]
            [fighorse.api.files :as files-api]
            [fighorse.export.images :as images]))

(def ^:private fs (js/require "fs"))
(def ^:private os (js/require "os"))
(def ^:private path (js/require "path"))

(defn- tmp-dir [prefix]
  (.mkdtempSync fs (.join path (.tmpdir os) prefix)))

(defn- restore-cwd! [cwd]
  (.chdir js/process cwd))

(defn- install-download-stubs! [content-type writes]
  (let [original-fetch js/fetch
        original-write (.-write js/Bun)]
    (set! js/fetch
          (fn [_url]
            (js/Promise.resolve
              #js {:ok true
                   :status 200
                   :blob (fn []
                           (js/Promise.resolve #js {:type content-type}))})))
    (set! (.-write js/Bun)
          (fn [dest-path _blob]
            (swap! writes conj dest-path)
            (js/Promise.resolve nil)))
    (fn []
      (set! js/fetch original-fetch)
      (set! (.-write js/Bun) original-write))))

(deftest export-images-sanitizes-node-filenames-and-writes-manifest
  (testing "node slices are safe for Android/iOS asset workflows"
    (async done
      (let [cwd (.cwd js/process)
            project (tmp-dir "fighorse-images-")
            dir (.join path project ".fighorse" "exports")
            writes (atom [])
            restore! (install-download-stubs! "image/png" writes)]
        (.chdir js/process project)
        (with-redefs [files-api/get-images
                      (fn [_token _file-key _node-ids & _opts]
                        (js/Promise.resolve {:images {"376:12995" "https://example.test/frame.png"}}))]
          (-> (images/export-images "token" "file" ["376:12995"]
                                    :dest-dir dir
                                    :format "png"
                                    :scale 1
                                    :manifest true)
              (.then (fn [result]
                       (let [expected (.join path (.realpathSync fs dir) "376_12995.png")
                             manifest (js->clj
                                        (js/JSON.parse
                                          (.readFileSync fs (.join path dir "manifest.json") "utf8"))
                                        :keywordize-keys true)]
                         (is (= expected (get result "376:12995")))
                         (is (= [expected] @writes))
                         (is (= "fighorse.image_export" (:kind manifest)))
                         (is (= "376:12995" (get-in manifest [:entries 0 :node_id])))
                         (is (= expected (get-in manifest [:entries 0 :path]))))
                       (restore!)
                       (restore-cwd! cwd)
                       (done)))
              (.catch (fn [err]
                        (restore!)
                        (restore-cwd! cwd)
                        (is false (str "export failed: " err))
                        (done)))))))))

(deftest download-image-fills-adds-extension-from-content-type
  (testing "image fills get useful filenames even when Figma URL has no extension"
    (async done
      (let [cwd (.cwd js/process)
            project (tmp-dir "fighorse-assets-")
            dir (.join path project ".fighorse" "exports")
            writes (atom [])
            restore! (install-download-stubs! "image/webp" writes)]
        (.chdir js/process project)
        (with-redefs [files-api/get-image-fills
                      (fn [_token _file-key]
                        (js/Promise.resolve {:meta {:images {"fill/ref" "https://example.test/fill"}}}))]
          (-> (images/download-image-fills "token" "file"
                                           :dest-dir dir
                                           :manifest true)
              (.then (fn [result]
                       (let [expected (.join path (.realpathSync fs dir) "fill_ref.webp")
                             manifest (js->clj
                                        (js/JSON.parse
                                          (.readFileSync fs (.join path dir "manifest.json") "utf8"))
                                        :keywordize-keys true)]
                         (is (= expected (get result "fill/ref")))
                         (is (= [expected] @writes))
                         (is (= "fighorse.asset_download" (:kind manifest)))
                         (is (= "fill/ref" (get-in manifest [:entries 0 :image_ref])))
                         (is (= expected (get-in manifest [:entries 0 :path]))))
                       (restore!)
                       (restore-cwd! cwd)
                       (done)))
              (.catch (fn [err]
                        (restore!)
                        (restore-cwd! cwd)
                        (is false (str "asset download failed: " err))
                        (done)))))))))

(deftest export-images-rejects-directories-outside-approved-roots
  (testing "exports cannot escape the project or fighorse export roots"
    (async done
      (let [cwd (.cwd js/process)
            project (tmp-dir "fighorse-export-policy-")
            outside (.join path project ".." "outside")
            writes (atom [])
            restore! (install-download-stubs! "image/png" writes)]
        (.chdir js/process project)
        (with-redefs [files-api/get-images
                      (fn [_token _file-key _node-ids & _opts]
                        (js/Promise.resolve {:images {"1:2" "https://example.test/frame.png"}}))]
          (-> (images/export-images "token" "file" ["1:2"]
                                    :dest-dir outside)
              (.then (fn [_]
                       (is false "export should reject unsafe directories")))
              (.catch (fn [err]
                        (is (re-find #"outside allowed roots" (.-message err)))
                        (is (empty? @writes))))
              (.finally (fn []
                          (restore!)
                          (restore-cwd! cwd)
                          (done)))))))))

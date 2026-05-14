(ns fighorse.experience-test
  (:require [cljs.test :refer [deftest is testing]]
            [clojure.string :as str]
            [fighorse.experience :as experience]))

(def ^:private fs (js/require "fs"))
(def ^:private os (js/require "os"))
(def ^:private path (js/require "path"))

(defn- with-temp-store [f]
  (let [original (.-FIGHORSE_EXPERIENCE_PATH js/process.env)
        dir (.mkdtempSync fs (.join path (.tmpdir os) "fighorse-experience-"))
        store (.join path dir "experience.jsonl")]
    (set! (.-FIGHORSE_EXPERIENCE_PATH js/process.env) store)
    (try
      (f store)
      (finally
        (if (nil? original)
          (js-delete js/process.env "FIGHORSE_EXPERIENCE_PATH")
          (set! (.-FIGHORSE_EXPERIENCE_PATH js/process.env) original))))))

(deftest add-list-and-summarize-experience
  (testing "stores versioned lessons and returns prompt-ready guidance"
    (with-temp-store
      (fn [store]
        (let [write (experience/add! {:summary "Compose rows overlapped"
                                      :lesson "Use Column or LazyColumn for repeated rows; Box stacks children."
                                      :category "layout"
                                      :severity "warning"
                                      :platform "android-compose"
                                      :asset_format "png"
                                      :tags "compose,overlap"
                                      :evidence "Real device screenshot showed duplicated text."
                                      :recommendation "Inspect repeated children before implementing a list."
                                      :client "codex"
                                      :command "record_experience"})
              listed (experience/list-experiences :platform "android-compose"
                                                  :asset-format "png"
                                                  :tag "compose"
                                                  :limit 4)
              guidance (experience/guidance :platform "android-compose"
                                            :asset-format "png")
              markdown (experience/guidance->markdown guidance)]
          (is (= "fighorse.experience-write.v1" (:kind write)))
          (is (= store (:store_path write)))
          (is (= experience/record-kind (get-in write [:record :kind])))
          (is (= 1 (get-in write [:record :schema_version])))
          (is (= "android-compose" (get-in write [:record :target :platform])))
          (is (.existsSync fs store))
          (is (= "fighorse.experience-summary.v1" (:kind listed)))
          (is (= 1 (:total_count listed)))
          (is (= 1 (:returned_count listed)))
          (is (= "Compose rows overlapped" (get-in listed [:records 0 :summary])))
          (is (= "fighorse.learned-guidance.v1" (:kind guidance)))
          (is (= "fighorse.experience-schema.v1" (get-in guidance [:schema :kind])))
          (is (= "fighorse.ai-contract.v1" (get-in guidance [:ai_contract :kind])))
          (is (some #(str/includes? % "manifest")
                    (get-in guidance [:ai_contract :must])))
          (is (some #(str/includes? % ".fighorse/exports")
                    (get-in guidance [:output_locations :export_policy :rules])))
          (is (seq (:record_when guidance)))
          (is (str/includes? markdown "Compose rows overlapped")))))))

(deftest experience-filters-are-compatible-with-global-records
  (testing "records without target filters still apply globally"
    (with-temp-store
      (fn [_store]
        (experience/add! {:summary "Always compare screenshots"
                          :lesson "Run the app and compare against the Figma screenshot before finalizing."})
        (let [listed (experience/list-experiences :platform "ios-swiftui"
                                                  :asset-format "pdf")]
          (is (= 1 (:returned_count listed)))
          (is (= "workflow" (get-in listed [:records 0 :category]))))))))

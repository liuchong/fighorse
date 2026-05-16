(ns fighorse.mcp.resources-test
  (:require [cljs.test :refer [deftest is testing]]
            [fighorse.mcp.resources :as resources]))

(deftest resources-expose-capabilities-coverage-workflow-and-experience
  (testing "resource list is discoverable"
    (let [uris (set (map :uri (:resources (resources/list-resources))))]
      (is (contains? uris "fighorse://capabilities"))
      (is (contains? uris "fighorse://coverage"))
      (is (contains? uris "fighorse://workflow/design-replication"))
      (is (contains? uris "fighorse://experience/summary"))))
  (testing "coverage resource returns JSON text"
    (let [result (resources/read-resource "fighorse://coverage")
          text (get-in result [:contents 0 :text])
          data (js->clj (js/JSON.parse text) :keywordize-keys true)]
      (is (= "fighorse.api-coverage.v1" (:kind data)))
      (is (= 48 (get-in data [:summary :operation_count]))))))

(deftest prompts-include-design-replication-workflow
  (testing "prompt list includes fighorse workflow prompts"
    (let [names (set (map :name (:prompts (resources/list-prompts))))]
      (is (contains? names "fighorse_design_replication"))
      (is (contains? names "fighorse_api_coverage"))))
  (testing "prompt body tells AI to use the learning loop"
    (let [prompt (resources/get-prompt "fighorse_design_replication"
                                       {:figma_url "https://www.figma.com/design/abc/Mock?node-id=1-2"
                                        :platform "web-react"
                                        :asset_format "svg"})
          text (get-in prompt [:messages 0 :content :text])]
      (is (string? text))
      (is (re-find #"record reusable lessons" text)))))

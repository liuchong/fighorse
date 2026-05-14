(ns fighorse.tokens-test
  (:require [cljs.test :refer [deftest is testing]]
            [fighorse.compact :as compact]
            [fighorse.tokens :as tokens]))

(def sample-tree
  {:id "0:1" :name "Page" :type "PAGE"
   :children [{:id "1:1" :name "Primary" :type "FRAME"
               :fills [{:type "SOLID" :color {:r 0.2 :g 0.4 :b 0.8 :a 1}}]
               :layout {:itemSpacing 8 :paddingLeft 16}
               :children [{:id "1:2" :name "Title" :type "TEXT"
                           :characters "Hello" :style {:fontFamily "Inter" :fontSize 24}}]}
              {:id "1:3" :name "Card" :type "FRAME"
               :fills [{:type "SOLID" :color {:r 1 :g 1 :b 1}}]
               :effects [{:type "DROP_SHADOW" :color {:r 0 :g 0 :b 0 :a 0.1}
                          :offset {:x 0 :y 2} :radius 4 :spread 0}]}]})

(deftest extract-tokens-test
  (testing "extracts colors from fills"
    (let [result (tokens/extract-tokens sample-tree)]
      (is (seq result))
      (let [colors (filter #(= "color" (:type %)) result)]
        (is (= 2 (count colors)))
        (is (= "Primary" (:name (first colors))))
        (is (= "#3366cc" (:hex (first colors)))))))
  (testing "extracts typography from text nodes"
    (let [result (tokens/extract-tokens sample-tree)]
      (let [typos (filter #(= "typography" (:type %)) result)]
        (is (= 1 (count typos)))
        (is (= "Title" (:name (first typos)))))))
  (testing "extracts typography from compacted textStyle"
    (let [simplified (compact/simplify-tree sample-tree compact/default-extractors 3)
          result (tokens/extract-tokens simplified)
          typos (filter #(= "typography" (:type %)) result)]
      (is (= 1 (count typos)))
      (is (= 24 (get-in (first typos) [:value :fontSize])))))
  (testing "extracts spacing from layout"
    (let [result (tokens/extract-tokens sample-tree)]
      (let [spacings (filter #(= "spacing" (:type %)) result)]
        (is (pos? (count spacings))))))
  (testing "extracts effects"
    (let [result (tokens/extract-tokens sample-tree)]
      (let [shadows (filter #(= "shadow" (:type %)) result)]
        (is (= 1 (count shadows)))
        (is (= "Card" (:name (first shadows))))))))

(deftest tokens-by-category-test
  (testing "groups tokens by type"
    (let [tokens (tokens/extract-tokens sample-tree)
          grouped (tokens/tokens-by-category tokens)]
      (is (contains? grouped "color"))
      (is (contains? grouped "typography"))
      (is (contains? grouped "spacing"))
      (is (contains? grouped "shadow")))))

(deftest format-tokens-test
  (testing "formats CSS variables"
    (let [css (tokens/format-tokens (tokens/extract-tokens sample-tree) :format "css")]
      (is (clojure.string/includes? css ":root"))
      (is (clojure.string/includes? css "--figma-color-primary: #3366cc;"))))
  (testing "formats Tailwind extension"
    (let [tw (tokens/format-tokens (tokens/extract-tokens sample-tree) :format "tailwind")]
      (is (= "#3366cc" (get-in tw [:theme :extend :colors "primary"]))))))

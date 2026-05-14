(ns fighorse.compact-test
  (:require [cljs.test :refer [deftest is testing]]
            [fighorse.compact :as compact]))

(def sample-node
  {:id "1:1"
   :name "Frame"
   :type "FRAME"
   :absoluteBoundingBox {:width 100 :height 200}
   :layoutMode "VERTICAL"
   :fills [{:type "SOLID" :color {:r 1 :g 0 :b 0} :visible "VISIBLE"}]
   :children [{:id "1:2" :name "Text" :type "TEXT"
              :characters "Hello" :style {:fontSize 16}}
             {:id "1:3" :name "Rect" :type "RECTANGLE"
              :absoluteBoundingBox {:width 50 :height 50}}]})

(deftest simplify-node-test
  (testing "simplify-node extracts key fields"
    (let [result (compact/simplify-node sample-node compact/default-extractors)]
      (is (= "1:1" (:id result)))
      (is (= "Frame" (:name result)))
      (is (= "FRAME" (:type result)))
      (is (some? (:layout result)))
      (is (some? (:dimensions result)))))
  (testing "text extractor captures characters and style"
    (let [text-node {:id "1:2" :name "Text" :type "TEXT"
                     :characters "Hello" :style {:fontSize 16}}
          result (compact/simplify-node text-node compact/default-extractors)]
      (is (= "Hello" (:characters result)))
      (is (= 16 (get-in result [:textStyle :fontSize]))))))

(deftest simplify-tree-test
  (testing "simplify-tree preserves hierarchy"
    (let [result (compact/simplify-tree sample-node compact/default-extractors 2)]
      (is (= "FRAME" (:type result)))
      (is (= 2 (count (:children result))))
      (is (= "TEXT" (get-in result [:children 0 :type])))))
  (testing "depth limit stops traversal"
    (let [result (compact/simplify-tree sample-node compact/default-extractors 0)]
      (is (nil? (:children result))))))

(deftest estimate-tokens-test
  (testing "token estimation is positive"
    (is (pos? (compact/estimate-tokens {:a 1 :b 2}))))
  (testing "larger data has more tokens"
    (is (> (compact/estimate-tokens {:a 1 :b 2 :c 3})
           (compact/estimate-tokens {:a 1})))))

(deftest score-tree-test
  (testing "score-tree adds :_score to nodes"
    (let [[scored _] (compact/score-tree sample-node)]
      (is (number? (:_score scored)))
      (is (pos? (:_score scored)))))
  (testing "children are vectors after scoring"
    (let [[scored _] (compact/score-tree sample-node)]
      (is (vector? (:children scored))))))

(deftest all-subtrees-path-test
  (testing "all-subtrees returns correct paths"
    (let [tree {:children [{:id "1" :name "A" :type "FRAME"
                            :children [{:id "2" :name "B" :type "TEXT"}]}
                           {:id "3" :name "C" :type "RECTANGLE"}]}
          [scored _] (compact/score-tree tree)
          subs (compact/all-subtrees scored [])]
      (is (= 4 (count subs)))
      (is (= [] (first (first subs))))
      (is (= [:children 0] (first (second subs))))
      (is (= [:children 0 :children 0] (first (nth subs 2))))
      (is (= [:children 1] (first (nth subs 3)))))))

(deftest truncate-at-path-test
  (testing "truncate-at-path replaces subtree"
    (let [tree {:children [{:id "1" :name "A" :type "FRAME"
                            :children [{:id "2" :name "B" :type "TEXT"}]}
                           {:id "3" :name "C" :type "RECTANGLE"}]}
          [scored _] (compact/score-tree tree)
          result (compact/truncate-at-path scored [:children 0])]
      (is (= 2 (count (:children result))))
      (is (= true (get-in result [:children 0 :truncated])))
      (is (= "A" (get-in result [:children 0 :name])))
      (is (= "C" (get-in result [:children 1 :name]))))))

(deftest truncate-by-budget-test
  (testing "tree within budget is not truncated"
    (let [tree (compact/simplify-tree sample-node compact/default-extractors 2)
          result (compact/truncate-by-budget tree 100000)]
      (is (nil? (:truncated result)))))
  (testing "tree exceeding budget gets truncated"
    ;; Build a larger tree to ensure truncation is triggered
    (let [big-tree {:id "0:1" :name "Page" :type "PAGE"
                    :children (vec (for [i (range 20)]
                                     {:id (str "1:" i) :name (str "Frame " i) :type "FRAME"
                                      :absoluteBoundingBox {:width 100 :height 100}
                                      :layout {:layoutMode "VERTICAL"}
                                      :fills [{:type "SOLID" :color {:r 1 :g 0 :b 0}}]
                                      :children [{:id (str "2:" i) :name (str "Text " i) :type "TEXT"
                                                  :characters (str "Hello " i)}]}))}
          tree (compact/simplify-tree big-tree compact/default-extractors 2)
          original-tokens (compact/estimate-tokens tree)
          result (compact/truncate-by-budget tree 100)]
      (is (some? result))
      ;; Truncation replaces subtrees with markers, so children count stays the same
      ;; but some should be marked as truncated and total tokens should decrease
      (is (some :truncated (:children result)))
      (let [final-tokens (compact/estimate-tokens result)]
        (is (< final-tokens original-tokens))))))

(deftest truncate-score-priority-test
  (testing "lower-score decorative nodes are truncated before higher-score text nodes"
    (let [tree {:id "0" :name "Root" :type "FRAME"
                :children [{:id "1" :name "Decor" :type "VECTOR"
                            :fills [{:type "SOLID" :color {:r 1 :g 0 :b 0}}]
                            :children (vec (for [i (range 10)]
                                             {:id (str "d" i)
                                              :name (str "Tiny " i)
                                              :type "VECTOR"}))}
                           {:id "2" :name "Copy" :type "TEXT"
                            :characters "Important call to action"}]}
          result (compact/truncate-by-budget tree (- (compact/estimate-tokens tree) 20))]
      (is (or (= true (get-in result [:children 0 :truncated]))
              (some :truncated (get-in result [:children 0 :children]))))
      (is (not= true (get-in result [:children 1 :truncated]))))))

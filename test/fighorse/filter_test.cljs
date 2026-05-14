(ns fighorse.filter-test
  (:require [cljs.test :refer [deftest is testing]]
            [fighorse.filter :as filter]))

(def sample-tree
  {:id "0" :name "Page" :type "PAGE"
   :children [{:id "1" :name "Login Card" :type "FRAME"
               :dimensions {:width 320 :height 200}
               :children [{:id "2" :name "Title" :type "TEXT" :visible true}
                          {:id "3" :name "Hidden" :type "TEXT" :visible false}]}
              {:id "4" :name "Dot" :type "RECTANGLE"
               :dimensions {:width 2 :height 2}}]})

(deftest parse-size-test
  (testing "parses width x height strings"
    (is (= {:width 10 :height 20} (filter/parse-size "10x20"))))
  (testing "invalid sizes return nil"
    (is (nil? (filter/parse-size "bad")))))

(deftest filter-tree-test
  (testing "keeps matching descendants and their ancestors"
    (let [result (filter/filter-tree sample-tree {:types #{"TEXT"} :visible-only true})]
      (is (= "Page" (:name result)))
      (is (= "Login Card" (get-in result [:children 0 :name])))
      (is (= ["Title"] (mapv :name (get-in result [:children 0 :children]))))))
  (testing "filters by minimum size"
    (let [result (filter/filter-tree sample-tree {:min-size {:width 100 :height 100}})]
      (is (= ["Login Card"] (mapv :name (:children result)))))))

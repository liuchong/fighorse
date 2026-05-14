(ns fighorse.diff-test
  (:require [cljs.test :refer [deftest is testing]]
            [fighorse.diff :as diff]))

(def old-tree
  {:id "0" :name "Page" :type "PAGE"
   :children [{:id "1" :name "Header" :type "FRAME"
               :children [{:id "2" :name "Title" :type "TEXT" :characters "Old"}]}
              {:id "3" :name "Removed" :type "RECTANGLE"}]})

(def new-tree
  {:id "0" :name "Page" :type "PAGE"
   :children [{:id "1" :name "Header" :type "FRAME"
               :children [{:id "2" :name "Title" :type "TEXT" :characters "New"}]}
              {:id "4" :name "Added" :type "RECTANGLE"}]})

(deftest diff-nodes-test
  (testing "reports immediate added, removed, and unchanged nodes"
    (let [result (diff/diff-nodes old-tree new-tree)]
      (is (= ["4"] (mapv :id (:added result))))
      (is (= ["3"] (mapv :id (:removed result))))
      (is (= ["Header"] (:unchanged result))))))

(deftest diff-trees-test
  (testing "recurses into common children"
    (let [result (diff/diff-trees old-tree new-tree)
          header-diff (first (:children result))
          title-change (first (:modified header-diff))]
      (is (= "1" (:id header-diff)))
      (is (= "2" (:id title-change)))
      (is (= "Old" (get-in title-change [:before :characters])))
      (is (= "New" (get-in title-change [:after :characters]))))))

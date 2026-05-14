(ns fighorse.export.md-test
  (:require [cljs.test :refer [deftest is testing]]
            [fighorse.export.md :as md]))

(def sample-tree
  {:id "0:1" :name "My Design" :type "PAGE"
   :dimensions {:width 375 :height 812}
   :children [{:id "1:1" :name "Header" :type "FRAME"
               :dimensions {:width 375 :height 60}
               :layout {:layoutMode "HORIZONTAL" :itemSpacing 8}
               :children [{:id "1:2" :name "Logo" :type "VECTOR"}
                          {:id "1:3" :name "Title" :type "TEXT"
                           :characters "App Name"}]}]})

(deftest tree->markdown-test
  (testing "generates markdown with headings"
    (let [result (md/tree->markdown sample-tree :title "Test")]
      (is (clojure.string/includes? result "# Test"))
      (is (clojure.string/includes? result "## Header"))
      (is (clojure.string/includes? result "### Logo"))))
  (testing "includes dimensions when present"
    (let [result (md/tree->markdown sample-tree)]
      (is (clojure.string/includes? result "375×812"))))
  (testing "includes text content"
    (let [result (md/tree->markdown sample-tree)]
      (is (clojure.string/includes? result "App Name")))))

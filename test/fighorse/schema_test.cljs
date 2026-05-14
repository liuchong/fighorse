(ns fighorse.schema-test
  (:require [cljs.test :refer [deftest is testing]]
            [fighorse.schema :as schema]))

(def component-tree
  {:id "0" :name "Page" :type "PAGE"
   :children [{:id "1" :name "Button/Primary" :type "COMPONENT"
               :componentProperties {"Disabled" {:type "BOOLEAN" :value false}
                                     "Label" {:type "TEXT" :value "Submit"}
                                     "Size" {:type "VARIANT" :value "md"
                                             :variantOptions ["sm" "md" "lg"]}}}]})

(deftest infer-component-schema-test
  (testing "infers component props"
    (let [result (schema/infer-component-schema component-tree "1")]
      (is (= "ButtonPrimaryProps" (:interface result)))
      (is (= 3 (count (:props result))))
      (is (= "boolean" (:type (first (:props result)))))))
  (testing "renders TypeScript interface"
    (let [ts (schema/schema->typescript (schema/infer-component-schema component-tree "1"))]
      (is (clojure.string/includes? ts "export interface ButtonPrimaryProps"))
      (is (clojure.string/includes? ts "\"Disabled\"?: boolean;")))))

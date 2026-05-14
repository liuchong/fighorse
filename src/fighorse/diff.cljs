(ns fighorse.diff
  "Figma file diff — compare two files or two versions.
   Returns a structural diff of nodes."
  (:require [clojure.set :as set]))

(defn- node-signature
  "Create a lightweight signature for diff comparison."
  [node]
  (select-keys node [:id :name :type :characters :visible]))

(defn- children-by-id [node]
  (if (seq (:children node))
    (into {} (map (juxt :id identity)) (:children node))
    {}))

(defn diff-nodes
  "Compare two node trees and return changes.
   Returns {:added [...] :removed [...] :modified [...] :unchanged [...]}"
  [old-node new-node]
  (let [old-children (children-by-id old-node)
        new-children (children-by-id new-node)
        old-ids (set (keys old-children))
        new-ids (set (keys new-children))
        added-ids (set/difference new-ids old-ids)
        removed-ids (set/difference old-ids new-ids)
        common-ids (set/intersection old-ids new-ids)]
    {:added (mapv #(get new-children %) added-ids)
     :removed (mapv #(get old-children %) removed-ids)
     :modified (vec (for [id common-ids
                          :let [old (get old-children id)
                                new (get new-children id)]
                          :when (not= (node-signature old) (node-signature new))]
                      {:id id
                       :name (:name new)
                       :type (:type new)
                       :before (node-signature old)
                       :after (node-signature new)}))
     :unchanged (vec (for [id common-ids
                           :let [old (get old-children id)
                                 new (get new-children id)]
                           :when (= (node-signature old) (node-signature new))]
                       (:name new)))}))

(defn diff-trees
  "Deep diff of two document trees. Returns recursive diff map."
  [old-tree new-tree]
  (let [base-diff (diff-nodes old-tree new-tree)
        old-children (children-by-id old-tree)
        new-children (children-by-id new-tree)
        common-ids (set/intersection (set (keys old-children))
                                     (set (keys new-children)))]
    (assoc base-diff :children
           (vec (for [id common-ids]
                  (assoc (diff-trees (get old-children id)
                                     (get new-children id))
                         :id id
                         :name (:name (get new-children id))
                         :type (:type (get new-children id))))))))

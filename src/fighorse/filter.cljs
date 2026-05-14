(ns fighorse.filter
  "Tree filtering utilities for AI-oriented Figma context selection."
  (:require [clojure.string :as str]))

(defn parse-size [s]
  (when (seq s)
    (let [[w h] (str/split s #"x")
          width (js/parseFloat w)
          height (js/parseFloat h)]
      (when (and (not (js/isNaN width))
                 (not (js/isNaN height)))
        {:width width :height height}))))

(defn- dimensions [node]
  (or (:dimensions node)
      (select-keys (:absoluteBoundingBox node) [:width :height])))

(defn- visible? [node]
  (not= false (:visible node)))

(defn- type-match? [node types]
  (or (empty? types)
      (contains? types (:type node))))

(defn- name-match? [node name-regex]
  (or (str/blank? name-regex)
      (.test (js/RegExp. name-regex "i") (or (:name node) ""))))

(defn- size-match? [node min-size]
  (if min-size
    (let [{:keys [width height]} (dimensions node)]
      (and (some? width)
           (some? height)
           (>= width (:width min-size))
           (>= height (:height min-size))))
    true))

(defn- node-match? [node {:keys [types name-regex visible-only min-size has-fill no-children]}]
  (and (type-match? node types)
       (name-match? node name-regex)
       (or (not visible-only) (visible? node))
       (size-match? node min-size)
       (or (not has-fill) (seq (:fills node)))
       (or (not no-children) (empty? (:children node)))))

(defn filter-tree
  "Filter a tree while preserving ancestors for matching descendants."
  [node opts]
  (when node
    (let [children (keep #(filter-tree % opts) (:children node))
          kept-children (vec children)
          self-match (node-match? node opts)]
      (when (or self-match (seq kept-children))
        (cond-> (dissoc node :children)
          (seq kept-children) (assoc :children kept-children))))))

(defn parse-types [s]
  (if (str/blank? s)
    #{}
    (set (map str/trim (str/split s #",")))))

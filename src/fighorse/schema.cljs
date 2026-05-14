(ns fighorse.schema
  "Component schema inference from Figma component nodes."
  (:require [clojure.string :as str]))

(defn find-node [node node-id]
  (when node
    (if (= node-id (:id node))
      node
      (some #(find-node % node-id) (:children node)))))

(defn- sanitize-name [s]
  (let [base (-> (or s "Component")
                 (str/replace #"[^A-Za-z0-9_ ]" " ")
                 (str/replace #"\s+" " ")
                 str/trim)]
    (if (str/blank? base)
      "Component"
      (str/replace base #" " ""))))

(defn- infer-property-type [prop]
  (case (:type prop)
    "BOOLEAN" "boolean"
    "TEXT" "string"
    "INSTANCE_SWAP" "string"
    "VARIANT" (if-let [options (:variantOptions prop)]
                (str/join " | " (map #(str "\"" % "\"") options))
                "string")
    "string"))

(defn infer-component-schema [tree component-id]
  (let [node (find-node tree component-id)
        props (:componentProperties node)]
    (when node
      {:component {:id (:id node)
                   :name (:name node)
                   :type (:type node)}
       :interface (str (sanitize-name (:name node)) "Props")
       :props (mapv (fn [[name prop]]
                      {:name name
                       :type (infer-property-type prop)
                       :default (:value prop)})
                    props)})))

(defn schema->typescript [{:keys [interface props]}]
  (let [lines (concat [(str "export interface " interface " {")]
                      (map (fn [{:keys [name type]}]
                             (str "  \"" name "\"?: " type ";"))
                           props)
                      ["}"])]
    (str/join "\n" lines)))

(ns fighorse.figma
  "Shared helpers for normalizing Figma REST responses.")

(defn response->node
  "Return the document/node payload from a Figma file or file-nodes response."
  [data]
  (if-let [nodes (:nodes data)]
    (let [docs (->> nodes vals (map :document) (remove nil?))]
      (if (= 1 (count docs))
        (first docs)
        {:id "selection"
         :name "Selection"
         :type "SELECTION"
         :children (vec docs)}))
    (or (:document data) data)))

(defn node-summary [node]
  (let [{:keys [width height]} (:absoluteBoundingBox node)]
    (cond-> {:id (:id node)
             :name (:name node)
             :type (:type node)
             :children_count (count (:children node))}
      (and width height) (assoc :dimensions {:width width :height height}))))

(defn renderable-node-ids
  "Pick node IDs suitable for screenshot rendering."
  [node explicit-node-id limit]
  (let [limit (or limit 4)
        root-id (:id node)
        root-type (:type node)
        child-ids (->> (:children node)
                       (filter #(contains? #{"CANVAS" "FRAME" "COMPONENT" "COMPONENT_SET" "INSTANCE"} (:type %)))
                       (map :id)
                       (remove nil?)
                       (take limit))]
    (cond
      explicit-node-id [explicit-node-id]
      (and root-id (not= "DOCUMENT" root-type)) [root-id]
      :else (vec child-ids))))

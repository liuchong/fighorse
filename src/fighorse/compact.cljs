(ns fighorse.compact
  "JSON simplification engine with token-budget-aware truncation.
   Inspired by Framelink's extractor pattern.
   All operations are immutable — input data is never modified.")

;; --- Token estimation ---

(defn estimate-tokens
  "Rough token estimation for JSON data.
   OpenAI models: ~1 token per 3.5 chars for structured JSON."
  [data]
  (max 1 (int (/ (count (pr-str data)) 3.5))))

;; --- Score function: how valuable is this node for AI? ---

(defn- node-info-score [node]
  (+ (case (:type node)
       ("FRAME" "COMPONENT" "COMPONENT_SET") 20
       ("INSTANCE") 15
       ("TEXT") 12
       ("RECTANGLE" "ELLIPSE" "STAR" "LINE" "REGULAR_POLYGON") 5
       ("VECTOR" "BOOLEAN_OPERATION") 2
       1)
     (if (seq (:children node)) 0 3)     ; leaf = concrete content
     (if (seq (:characters node)) 5 0))) ; text is valuable

(defn- score-tree [node]
  "Post-order traversal. Returns [scored-node total-score].
   Each node gets :_score = self-score + sum(children-scores)."
  (if (seq (:children node))
    (let [scored-children (map score-tree (:children node))
          child-scores (map second scored-children)
          self-score (node-info-score node)
          total (+ self-score (reduce + child-scores))]
      [(assoc node
              :children (into [] (map first scored-children))
              :_score total)
       total])
    [(assoc node :_score (node-info-score node))
     (node-info-score node)]))

;; --- Extractors ---

(defn- clean-fills [fills]
  (when (seq fills)
    (not-empty
     (into []
           (keep (fn [fill]
                   (when (= "VISIBLE" (or (:visible fill) "VISIBLE"))
                     (let [base (select-keys fill [:type :opacity])]
                       (case (:type fill)
                         "SOLID" (assoc base :color (select-keys (:color fill) [:r :g :b :a]))
                         ("GRADIENT_LINEAR" "GRADIENT_RADIAL" "GRADIENT_ANGULAR" "GRADIENT_DIAMOND")
                         (assoc base :gradientHandlePositions (:gradientHandlePositions fill)
                                     :gradientStops (:gradientStops fill))
                         "IMAGE" (assoc base :imageRef (:imageRef fill))
                         base)))))
           fills))))

(defn- clean-strokes [strokes]
  (when (seq strokes)
    (not-empty
     (into []
           (keep (fn [stroke]
                   (when (= "VISIBLE" (or (:visible stroke) "VISIBLE"))
                     (let [base (select-keys stroke [:type :strokeWeight :strokeAlign])]
                       (if (= "SOLID" (:type stroke))
                         (assoc base :color (select-keys (:color stroke) [:r :g :b :a]))
                         base)))))
           strokes))))

(defn layout-extractor [node]
  (when (or (= "FRAME" (:type node))
            (= "COMPONENT" (:type node))
            (= "INSTANCE" (:type node)))
    (let [layout (select-keys node
                              [:layoutMode :layoutAlign :layoutGrow
                               :layoutPositioning :layoutSizingHorizontal
                               :layoutSizingVertical :itemSpacing
                               :counterAxisSpacing :counterAxisAlignItems
                               :primaryAxisAlignItems :paddingLeft
                               :paddingRight :paddingTop :paddingBottom])]
      (when (seq layout)
        {:layout layout}))))

(defn text-extractor [node]
  (when (= "TEXT" (:type node))
    (let [base (select-keys node [:characters])]
      (when (seq base)
        (assoc base :textStyle (select-keys (:style node)
                                            [:fontFamily :fontSize :fontWeight
                                             :textAlignHorizontal :textAlignVertical
                                             :letterSpacing :lineHeightPx :lineHeightPercentFontSize]))))))

(defn visuals-extractor [node]
  (let [visuals (merge
                 (when-let [fills (clean-fills (:fills node))]
                   {:fills fills})
                 (when-let [strokes (clean-strokes (:strokes node))]
                   {:strokes strokes})
                 (when-let [effects (not-empty
                                      (into [] (keep #(when (= "VISIBLE" (or (:visible %) "VISIBLE"))
                                                        (select-keys % [:type :color :offset :radius :spread])))
                                            (:effects node)))]
                   {:effects effects})
                 (when (:opacity node)
                   {:opacity (:opacity node)})
                 (when (:cornerRadius node)
                   {:cornerRadius (:cornerRadius node)})
                 (when (:rectangleCornerRadii node)
                   {:rectangleCornerRadii (:rectangleCornerRadii node)}))]
    (when (seq visuals)
      visuals)))

(defn dimension-extractor [node]
  (when (:absoluteBoundingBox node)
    (let [{:keys [width height]} (:absoluteBoundingBox node)]
      {:dimensions {:width width :height height}})))

(defn component-extractor [node]
  (when (= "INSTANCE" (:type node))
    (select-keys node [:componentId :componentProperties])))

;; --- Simplification pipeline ---

(def ^:private default-extractors
  [layout-extractor
   text-extractor
   visuals-extractor
   dimension-extractor
   component-extractor])

(defn- apply-extractors [node extractors]
  (reduce (fn [result extractor]
            (if-let [data (extractor node)]
              (merge result data)
              result))
          {} extractors))

(defn simplify-node
  "Apply extractors to a single node. Returns a new immutable map."
  [node extractors]
  (merge
   {:id (:id node)
    :name (:name node)
    :type (:type node)}
   (apply-extractors node extractors)))

(defn- should-traverse? [node max-depth current-depth]
  (and (or (nil? max-depth)
           (< current-depth max-depth))
       (seq (:children node))
       (not= false (:visible node))))

(defn simplify-tree
  "Recursively simplify a node tree.
   Returns a new immutable tree with :_score on every node."
  ([node] (simplify-tree node default-extractors nil 0))
  ([node extractors] (simplify-tree node extractors nil 0))
  ([node extractors max-depth]
   (simplify-tree node extractors max-depth 0))
  ([node extractors max-depth current-depth]
   (if (nil? node)
     nil
     (let [base (simplify-node node extractors)]
       (if (should-traverse? node max-depth current-depth)
         (assoc base :children
                (into [] (map #(simplify-tree % extractors max-depth (inc current-depth)))
                      (:children node)))
         base)))))

;; --- Token-aware truncation ---

(defn all-subtrees [node path]
  "Collect all nodes with their vector path for update-in."
  (cons [path node]
        (when (seq (:children node))
          (mapcat (fn [child i]
                    (all-subtrees child (conj path :children i)))
                  (:children node)
                  (range)))))

(defn truncate-at-path [tree path]
  "Replace the subtree at path with a truncated marker."
  (let [node (get-in tree path)]
    (assoc-in tree path
              {:id (:id node)
               :name (:name node)
               :type (:type node)
               :truncated true})))

(defn- truncation-marker [node]
  {:id (:id node)
   :name (:name node)
   :type (:type node)
   :truncated true})

(defn- truncation-saving [node]
  (- (estimate-tokens node)
     (estimate-tokens (truncation-marker node))))

(defn- truncation-candidates [scored]
  (->> (rest (all-subtrees scored [])) ;; skip root
       (keep (fn [[path node]]
               (let [saving (truncation-saving node)]
                 (when (pos? saving)
                   {:path (vec path)
                    :node node
                    :saving saving}))))
       (sort-by (fn [{:keys [path node saving]}]
                  [(node-info-score node) (- saving) (- (count path))]))))

(defn truncate-by-budget
  "Smart truncation: remove lowest-score subtrees until within token budget.
   Returns tree with some subtrees replaced by {:truncated true}."
  [node max-tokens]
  (let [[scored _] (score-tree node)
        initial-tokens (estimate-tokens scored)]
    (if (<= initial-tokens max-tokens)
      scored
      (loop [current scored
             candidates (truncation-candidates scored)
             removed #{}
             iterations 0]
        (let [tokens (estimate-tokens current)]
          (if (or (<= tokens max-tokens)
                  (empty? candidates)
                  (> iterations 1000))
            current
            (let [{:keys [path]} (first candidates)]
              (if (or (removed path)
                      (nil? (get-in current path)))
                (recur current (rest candidates) removed iterations)
                (recur (truncate-at-path current path)
                       (rest candidates)
                       (conj removed path)
                       (inc iterations))))))))))

;; --- Convenience API ---

(defn compact
  "Full pipeline: simplify + optional token truncation.
   Options: :extractors :max-depth :max-tokens"
  [node & {:keys [extractors max-depth max-tokens]
           :or {extractors default-extractors}}]
  (let [simplified (simplify-tree node extractors max-depth)]
    (if max-tokens
      (truncate-by-budget simplified max-tokens)
      simplified)))

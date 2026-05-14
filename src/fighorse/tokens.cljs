(ns fighorse.tokens
  "Design token extraction from Figma files.
   Extracts colors, typography, spacing, effects as design tokens."
  (:require [clojure.string :as str]))

(defn- clamp-channel [v]
  (-> (or v 0)
      (* 255)
      js/Math.round
      (max 0)
      (min 255)))

(defn- channel->hex [v]
  (let [s (.toString (clamp-channel v) 16)]
    (if (= 1 (count s))
      (str "0" s)
      s)))

(defn- color->hex [{:keys [r g b]}]
  (when (and (some? r) (some? g) (some? b))
    (str "#" (channel->hex r) (channel->hex g) (channel->hex b))))

(defn- extract-colors [node tokens]
  (let [fills (:fills node)
        new-colors (keep (fn [fill]
                           (when (= "SOLID" (:type fill))
                             (let [{:keys [r g b a]} (get-in fill [:color])]
                               {:name (:name node)
                                :type "color"
                                :value {:r r :g g :b b :a (or a 1)}
                                :hex (color->hex (:color fill))})))
                         fills)]
    (into tokens new-colors)))

(defn- extract-typography [node tokens]
  (if (= "TEXT" (:type node))
    (let [style (or (:textStyle node) (:style node))]
      (if style
        (conj tokens
              {:name (:name node)
               :type "typography"
               :value (select-keys style [:fontFamily :fontSize :fontWeight
                                          :textAlignHorizontal :textAlignVertical
                                          :letterSpacing :lineHeightPx])})
        tokens))
    tokens))

(defn- extract-spacing [node tokens]
  (let [layout (:layout node)
        spacing-keys [:itemSpacing :counterAxisSpacing :paddingLeft
                      :paddingRight :paddingTop :paddingBottom]
        spacing-values (select-keys layout spacing-keys)]
    (if (seq spacing-values)
      (conj tokens
            {:name (:name node)
             :type "spacing"
             :value spacing-values})
      tokens)))

(defn- extract-effects [node tokens]
  (let [effects (:effects node)
        new-effects (keep (fn [effect]
                            (when (= "DROP_SHADOW" (:type effect))
                              {:name (:name node)
                               :type "shadow"
                               :value (select-keys effect [:type :color :offset :radius :spread])}))
                          effects)]
    (into tokens new-effects)))

(defn extract-tokens
  "Extract design tokens from a simplified Figma tree."
  ([tree] (extract-tokens tree []))
  ([tree tokens]
   (let [new-tokens (->> tokens
                         (extract-colors tree)
                         (extract-typography tree)
                         (extract-spacing tree)
                         (extract-effects tree))]
     (if (seq (:children tree))
       (reduce (fn [acc child] (extract-tokens child acc)) new-tokens (:children tree))
       new-tokens))))

(defn tokens-by-category
  "Group extracted tokens by category."
  [tokens]
  (group-by :type tokens))

(defn- token-key [prefix token suffix]
  (let [name-part (-> (:name token)
                      (or "token")
                      str/lower-case
                      (str/replace #"[^a-z0-9]+" "-")
                      (str/replace #"(^-|-$)" ""))]
    (str prefix (:type token) "-" name-part suffix)))

(defn- css-value [token]
  (case (:type token)
    "color" (:hex token)
    "spacing" (str/join " "
                        (map (fn [[k v]] (str (name k) ":" v "px"))
                             (:value token)))
    "typography" (str/join " "
                           (keep (fn [[k v]]
                                   (when (some? v)
                                     (str (name k) ":" v)))
                                 (:value token)))
    "shadow" (let [{:keys [color offset radius spread]} (:value token)
                   {:keys [r g b a]} color]
               (str (or (:x offset) 0) "px "
                    (or (:y offset) 0) "px "
                    (or radius 0) "px "
                    (or spread 0) "px "
                    "rgba(" (clamp-channel r) ", "
                    (clamp-channel g) ", "
                    (clamp-channel b) ", "
                    (or a 1) ")"))
    (pr-str (:value token))))

(defn tokens->css
  ([tokens] (tokens->css tokens "--figma-"))
  ([tokens prefix]
   (str ":root {\n"
        (str/join "\n"
                  (map #(str "  " (token-key prefix % "") ": " (css-value %) ";")
                       tokens))
        "\n}")))

(defn tokens->scss
  ([tokens] (tokens->scss tokens "$figma-"))
  ([tokens prefix]
   (str/join "\n"
             (map #(str (token-key prefix % "") ": " (css-value %) ";")
                  tokens))))

(defn tokens->tailwind [tokens]
  {:theme
   {:extend
    {:colors (into {}
                   (keep (fn [token]
                           (when (= "color" (:type token))
                             [(-> (:name token)
                                  str/lower-case
                                  (str/replace #"[^a-z0-9]+" "-"))
                              (:hex token)])))
                   tokens)}}})

(defn format-tokens
  [tokens & {:keys [format prefix]
             :or {format "json" prefix "--figma-"}}]
  (case format
    "css" (tokens->css tokens prefix)
    "scss" (tokens->scss tokens (if (= "--figma-" prefix) "$figma-" prefix))
    "tailwind" (tokens->tailwind tokens)
    tokens))

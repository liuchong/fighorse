(ns fighorse.export.md
  "Export Figma file structure as Markdown.
   Generates human-readable design documentation."
  (:require [clojure.string :as str]))

(defn- indent [level]
  (str/join "" (repeat level "  ")))

(defn- node->md
  "Convert a single node to Markdown fragment."
  [node level]
  (let [heading-level (min (+ level 2) 6)
        heading-prefix (str/join "" (repeat heading-level "#"))
        type-label (:type node)
        name (:name node)
        dims (get-in node [:dimensions])
        layout (:layout node)
        text (:characters node)]
    (cond-> [(str heading-prefix " " name " (`" type-label "`)")]
      dims (conj (str (indent level) "- **Dimensions**: " (:width dims) "×" (:height dims)))
      layout (conj (str (indent level) "- **Layout**: " (pr-str (select-keys layout [:layoutMode :itemSpacing :paddingLeft :paddingRight :paddingTop :paddingBottom]))))
      text (conj (str (indent level) "- **Text**: \"" text "\""))
      (seq (:fills node)) (conj (str (indent level) "- **Fills**: " (count (:fills node)) " fill(s)"))
      (seq (:strokes node)) (conj (str (indent level) "- **Strokes**: " (count (:strokes node)) " stroke(s)"))
      (:truncated node) (conj (str (indent level) "- ⚠️ **Truncated**")))))

(defn- traverse->md [node level]
  (let [lines (node->md node level)]
    (if (seq (:children node))
      (into lines
            (mapcat #(traverse->md % (inc level)))
            (:children node))
      lines)))

(defn tree->markdown
  "Convert a Figma tree to Markdown document."
  [tree & {:keys [title]}]
  (let [header (str "# " (or title (:name tree) "Figma Design Document") "\n\n")
        body-lines (traverse->md tree 0)]
    (str header (str/join "\n" body-lines))))

(ns fighorse.api.comments
  "Figma Comments & Comment Reactions API — 6 endpoints.
   https://developers.figma.com/docs/rest-api/comments-endpoints/"
  (:require [fighorse.utils.http :as http]))

;; --- Comments ---

(defn get-comments
  "GET /v1/files/{file_key}/comments
   Gets a list of comments left on the file."
  [token file-key & {:keys [as-md]}]
  (http/get (str "/v1/files/" (http/path-segment file-key) "/comments") token
            :params {:as_md as-md}))

(defn post-comment
  "POST /v1/files/{file_key}/comments
   Posts a new comment on the file."
  [token file-key message & {:keys [comment-id client-meta]}]
  (http/post (str "/v1/files/" (http/path-segment file-key) "/comments") token
             :body {:message message
                    :comment_id comment-id
                    :client_meta client-meta}))

(defn delete-comment
  "DELETE /v1/files/{file_key}/comments/{comment_id}
   Deletes a specific comment."
  [token file-key comment-id]
  (http/delete (str "/v1/files/" (http/path-segment file-key) "/comments/" (http/path-segment comment-id)) token))

;; --- Comment Reactions ---

(defn get-comment-reactions
  "GET /v1/files/{file_key}/comments/{comment_id}/reactions
   Gets a paginated list of reactions left on the comment."
  [token file-key comment-id & {:keys [cursor]}]
  (http/get (str "/v1/files/" (http/path-segment file-key) "/comments/" (http/path-segment comment-id) "/reactions") token
            :params {:cursor cursor}))

(defn post-comment-reaction
  "POST /v1/files/{file_key}/comments/{comment_id}/reactions
   Posts a new comment reaction on a file comment."
  [token file-key comment-id emoji]
  (http/post (str "/v1/files/" (http/path-segment file-key) "/comments/" (http/path-segment comment-id) "/reactions") token
             :body {:emoji emoji}))

(defn delete-comment-reaction
  "DELETE /v1/files/{file_key}/comments/{comment_id}/reactions
   Deletes a specific comment reaction."
  [token file-key comment-id emoji]
  (http/delete (str "/v1/files/" (http/path-segment file-key) "/comments/" (http/path-segment comment-id) "/reactions") token
               :params {:emoji emoji}))

(ns fighorse.mcp.server-test
  (:require [cljs.test :refer [deftest is testing]]
            [clojure.string :as str]
            [fighorse.mcp.server :as server]))

(def ^:private Buffer (.-Buffer js/globalThis))
(def ^:private fs (js/require "fs"))
(def ^:private os (js/require "os"))
(def ^:private path (js/require "path"))

(defn- buffer [s]
  (.from Buffer s "utf8"))

(deftest stdio-parser-supports-line-and-header-framing
  (testing "newline-delimited JSON framing"
    (let [body (js/JSON.stringify #js {:jsonrpc "2.0"
                                        :id 1
                                        :method "initialize"})
          state (atom {:buffer (buffer (str body "\n"))
                       :mode nil})
          message (server/parse-stdio-message! state)]
      (is (= "initialize" (aget message "method")))
      (is (= "line" (:mode @state)))
      (is (nil? (:buffer @state)))))
  (testing "Content-Length framing"
    (let [body (js/JSON.stringify #js {:jsonrpc "2.0"
                                        :id 2
                                        :method "tools/list"})
          frame (str "Content-Length: " (.byteLength Buffer body "utf8") "\r\n\r\n" body)
          state (atom {:buffer (buffer frame)
                       :mode nil})
          message (server/parse-stdio-message! state)]
      (is (= "tools/list" (aget message "method")))
      (is (= "header" (:mode @state)))
      (is (nil? (:buffer @state))))))

(deftest stdio-parser-waits-for-split-header
  (let [body (js/JSON.stringify #js {:jsonrpc "2.0"
                                     :id 3
                                     :method "tools/list"})
        header (str "Content-Length: " (.byteLength Buffer body "utf8") "\r\n\r\n")
        state (atom {:buffer (buffer "Content-Length: ")
                     :mode nil})]
    (is (= :fighorse.mcp.server/no-message (server/parse-stdio-message! state)))
    (swap! state update :buffer
           (fn [existing] (.concat Buffer #js [existing (buffer (str (subs header 16) body))])))
    (let [message (server/parse-stdio-message! state)]
      (is (= "tools/list" (aget message "method")))
      (is (= "header" (:mode @state))))))

(deftest stdio-serializer-matches-selected-framing
  (let [message #js {:jsonrpc "2.0"
                     :id 1
                     :result #js {:text "中文"}}
        header-payload (server/serialize-stdio-message message "header")
        [_ header body] (re-find #"(?s)^(.*?)\r\n\r\n(.*)$" header-payload)
        length (js/parseInt (second (re-find #"Content-Length: (\d+)" header)) 10)]
    (is (= length (.byteLength Buffer body "utf8")))
    (is (= "中文" (aget (aget (js/JSON.parse body) "result") "text")))
    (is (str/ends-with? (server/serialize-stdio-message message "line") "\n"))))

(deftest stdio-polling-is-opt-in
  (let [original (.-FIGHORSE_MCP_STDIO_POLL js/process.env)]
    (try
      (js-delete js/process.env "FIGHORSE_MCP_STDIO_POLL")
      (is (false? (server/stdio-poll-enabled?)))
      (set! (.-FIGHORSE_MCP_STDIO_POLL js/process.env) "1")
      (is (true? (server/stdio-poll-enabled?)))
      (set! (.-FIGHORSE_MCP_STDIO_POLL js/process.env) "true")
      (is (true? (server/stdio-poll-enabled?)))
      (set! (.-FIGHORSE_MCP_STDIO_POLL js/process.env) "0")
      (is (false? (server/stdio-poll-enabled?)))
      (finally
        (if (nil? original)
          (js-delete js/process.env "FIGHORSE_MCP_STDIO_POLL")
          (set! (.-FIGHORSE_MCP_STDIO_POLL js/process.env) original))))))

(deftest mcp-singleton-lock-rejects-active-owner-and-cleans-stale-lock
  (let [original-lock (.-FIGHORSE_MCP_LOCK_FILE js/process.env)
        original-multiple (.-FIGHORSE_MCP_ALLOW_MULTIPLE js/process.env)
        dir (.mkdtempSync fs (.join path (.tmpdir os) "fighorse-lock-"))
        lock-file (.join path dir "mcp.lock")]
    (set! (.-FIGHORSE_MCP_LOCK_FILE js/process.env) lock-file)
    (js-delete js/process.env "FIGHORSE_MCP_ALLOW_MULTIPLE")
    (try
      (let [lock (server/acquire-singleton-lock! "sse" 9449)]
        (is (.existsSync fs lock-file))
        (is (thrown-with-msg? js/Error #"already running"
                              (server/acquire-singleton-lock! "stdio" 9449)))
        (server/release-singleton-lock! lock)
        (is (not (.existsSync fs lock-file))))
      (.writeFileSync fs lock-file (js/JSON.stringify #js {:pid 999999999}))
      (let [lock (server/acquire-singleton-lock! "sse" 9449)]
        (is (= (.-pid js/process) (:pid lock)))
        (server/release-singleton-lock! lock))
      (finally
        (if (nil? original-lock)
          (js-delete js/process.env "FIGHORSE_MCP_LOCK_FILE")
          (set! (.-FIGHORSE_MCP_LOCK_FILE js/process.env) original-lock))
        (if (nil? original-multiple)
          (js-delete js/process.env "FIGHORSE_MCP_ALLOW_MULTIPLE")
          (set! (.-FIGHORSE_MCP_ALLOW_MULTIPLE js/process.env) original-multiple))))))

(deftest stdio-parser-rejects-oversized-header-message
  (let [original (.-FIGHORSE_MCP_STDIO_MAX_BYTES js/process.env)
        body "{}"
        frame (str "Content-Length: 3\r\n\r\n" body)
        state (atom {:buffer (buffer frame)
                     :mode nil})]
    (set! (.-FIGHORSE_MCP_STDIO_MAX_BYTES js/process.env) "2")
    (try
      (is (thrown-with-msg? js/Error #"exceeds max size"
                            (server/parse-stdio-message! state)))
      (finally
        (if (nil? original)
          (js-delete js/process.env "FIGHORSE_MCP_STDIO_MAX_BYTES")
          (set! (.-FIGHORSE_MCP_STDIO_MAX_BYTES js/process.env) original))))))

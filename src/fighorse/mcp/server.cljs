(ns fighorse.mcp.server
  "MCP Server implementation using @modelcontextprotocol/sdk.
   Supports SSE and stdio transports for integration with MCP clients."
  (:require [clojure.string :as str]
            [fighorse.config :as config]
            [fighorse.discovery :as discovery]
            [fighorse.mcp.resources :as resources]
            [fighorse.mcp.tools :as tools]))

(def ^:private Buffer (.-Buffer js/globalThis))
(def ^:private fs (js/require "fs"))
(def ^:private path (js/require "path"))
(def ^:private default-stdio-max-bytes 10485760)

(def ^:private no-message ::no-message)
(def ^:private skip-message ::skip-message)
(defonce ^:private poll-state (atom ::uninitialized))
(defonce ^:private shutdown-state (atom {:installed false
                                         :closing false
                                         :lock nil
                                         :close nil}))

(defn- stdio-debug [& parts]
  (when (= "1" (.-FIGHORSE_MCP_STDIO_DEBUG js/process.env))
    (js/console.error (str/join " " (map str parts)))))

(defn- load-poll []
  (try
    (let [ffi (js/require "bun:ffi")
          FFIType (.-FFIType ffi)
          lib-name (case (.-platform js/process)
                     "darwin" "/usr/lib/libSystem.B.dylib"
                     "linux" "libc.so.6"
                     nil)]
      (when lib-name
        (let [^js lib ((.-dlopen ffi)
                       lib-name
                       #js {:poll #js {:args #js [(.-ptr FFIType) (.-i32 FFIType) (.-i32 FFIType)]
                                        :returns (.-i32 FFIType)}})
              ^js symbols (.-symbols lib)]
          {:poll (.-poll symbols)
           :ptr (.-ptr ffi)})))
    (catch :default _
      nil)))

(defn- poll-runtime []
  (let [current @poll-state]
    (if (= current ::uninitialized)
      (let [loaded (load-poll)]
        (reset! poll-state loaded)
        loaded)
      current)))

(defn- fd-readable? [fd]
  (if-let [{:keys [poll ptr]} (poll-runtime)]
    (let [buffer (js/ArrayBuffer. 8)
          view (js/DataView. buffer)]
      (.setInt32 view 0 fd true)
      (.setInt16 view 4 1 true)
      (.setInt16 view 6 0 true)
      (pos? (poll (ptr buffer) 1 0)))
    false))

(defn- concat-buffer [current chunk]
  (if current
    (.concat Buffer #js [current chunk])
    chunk))

(defn- remaining-buffer [buffer start]
  (let [next-buffer (.subarray buffer start)]
    (when (pos? (.-length next-buffer))
      next-buffer)))

(defn- first-header-separator [buffer]
  (let [crlf (.indexOf buffer "\r\n\r\n")
        lf (.indexOf buffer "\n\n")
        candidates (remove neg? [crlf lf])]
    (when (seq candidates)
      (let [idx (apply min candidates)]
        {:idx idx
         :len (if (= idx crlf) 4 2)}))))

(defn- stdio-max-bytes []
  (let [raw (.-FIGHORSE_MCP_STDIO_MAX_BYTES js/process.env)
        parsed (when raw (js/parseInt raw 10))]
    (if (and parsed (not (js/isNaN parsed)) (pos? parsed))
      parsed
      default-stdio-max-bytes)))

(defn stdio-poll-enabled?
  "Polling stdin is kept as an opt-in compatibility mode because it can burn CPU
   when an MCP server is idle under Bun's compiled runtime."
  []
  (contains? #{"1" "true"} (some-> (.-FIGHORSE_MCP_STDIO_POLL js/process.env)
                                   str/lower-case)))

(defn mcp-multiple-enabled?
  "Allow multiple MCP server processes only when explicitly requested."
  []
  (contains? #{"1" "true"} (some-> (.-FIGHORSE_MCP_ALLOW_MULTIPLE js/process.env)
                                   str/lower-case)))

(defn mcp-lock-file []
  (or (.-FIGHORSE_MCP_LOCK_FILE js/process.env)
      (.join path (config/fighorse-home) "runtime" "mcp.lock")))

(defn- active-pid? [pid]
  (and (number? pid)
       (pos? pid)
       (try
         (.kill js/process pid 0)
         true
         (catch :default err
           (not= "ESRCH" (.-code err))))))

(defn- read-lock-file [file]
  (try
    (js->clj (js/JSON.parse (.readFileSync fs file "utf8")) :keywordize-keys true)
    (catch :default _
      nil)))

(defn- write-lock-file! [file data]
  (.mkdirSync fs (.dirname path file) #js {:recursive true})
  (let [fd (.openSync fs file "wx")]
    (try
      (.writeFileSync fs fd (js/JSON.stringify (clj->js data) nil 2))
      (finally
        (.closeSync fs fd)))))

(defn acquire-singleton-lock!
  "Acquire the process-wide MCP server lock. Stale lock files are removed."
  [transport port]
  (when-not (mcp-multiple-enabled?)
    (let [file (mcp-lock-file)
          pid (.-pid js/process)
          data {:kind "fighorse.mcp-lock.v1"
                :pid pid
                :transport transport
                :port port
                :started_at (.toISOString (js/Date.))}]
      (loop [attempt 0]
        (let [result (try
                       (write-lock-file! file data)
                       {:file file :pid pid}
                       (catch :default err
                         (if (and (= "EEXIST" (.-code err)) (zero? attempt))
                           (let [existing (read-lock-file file)
                                 existing-pid (:pid existing)]
                             (if (active-pid? existing-pid)
                               (throw (js/Error. (str "Another fighorse MCP server is already running"
                                                      (when existing-pid (str " (pid " existing-pid ")"))
                                                      ". Stop it first or set FIGHORSE_MCP_ALLOW_MULTIPLE=1 for development.")))
                               (do
                                 (try
                                   (.unlinkSync fs file)
                                   (catch :default _ nil))
                                 ::retry)))
                           (throw err))))]
          (if (= ::retry result)
            (recur (inc attempt))
            result))))))

(defn release-singleton-lock! [lock]
  (when-let [file (:file lock)]
    (try
      (let [existing (read-lock-file file)]
        (when (= (:pid existing) (:pid lock))
          (.unlinkSync fs file)))
      (catch :default _
        nil))))

(defn- cleanup! []
  (when-let [lock (:lock @shutdown-state)]
    (release-singleton-lock! lock)
    (swap! shutdown-state assoc :lock nil)))

(defn- close-resource! [close-fn]
  (if close-fn
    (try
      (js/Promise.resolve (close-fn))
      (catch :default err
        (js/Promise.reject err)))
    (js/Promise.resolve)))

(defn- install-shutdown-handlers! [lock close-fn]
  (swap! shutdown-state assoc :lock lock :close close-fn)
  (when-not (:installed @shutdown-state)
    (swap! shutdown-state assoc :installed true)
    (.on js/process "exit" cleanup!)
    (doseq [signal ["SIGINT" "SIGTERM"]]
      (.once js/process signal
             (fn []
               (when-not (:closing @shutdown-state)
                 (swap! shutdown-state assoc :closing true)
                 (-> (close-resource! (:close @shutdown-state))
                     (.catch (fn [err]
                               (js/console.error "MCP shutdown error:" (or (.-message err) (str err)))))
                     (.finally (fn []
                                 (cleanup!)
                                 (js/process.exit 0))))))))))

(defn serialize-stdio-message
  "Serialize an MCP JSON-RPC message. Header mode is required by FastMCP/Codex-style clients;
   line mode preserves compatibility with older TypeScript SDK clients."
  [message mode]
  (let [json (js/JSON.stringify message)]
    (if (= "header" mode)
      (str "Content-Length: " (.byteLength Buffer json "utf8") "\r\n\r\n" json)
      (str json "\n"))))

(defn- parse-line-message! [state]
  (let [buffer (:buffer @state)
        idx (when buffer (.indexOf buffer "\n"))]
    (if (or (nil? idx) (= -1 idx))
      no-message
      (let [line (str/replace (.toString buffer "utf8" 0 idx) #"\r$" "")]
        (swap! state assoc
               :mode "line"
               :buffer (remaining-buffer buffer (inc idx)))
        (if (str/blank? line)
          skip-message
          (do
            (stdio-debug "stdio parsed line" line)
            (js/JSON.parse line)))))))

(defn- parse-header-message! [state]
  (let [buffer (:buffer @state)
        sep (when buffer (first-header-separator buffer))]
    (if-not sep
      no-message
      (let [header (.toString buffer "utf8" 0 (:idx sep))
            match (.match header (js/RegExp. "content-length:\\s*(\\d+)" "i"))]
        (when-not match
          (throw (js/Error. "Missing Content-Length header in MCP stdio message")))
        (let [content-length (js/parseInt (aget match 1) 10)
              body-start (+ (:idx sep) (:len sep))
              body-end (+ body-start content-length)]
          (when (> content-length (stdio-max-bytes))
            (throw (js/Error. (str "MCP stdio message exceeds max size: " content-length))))
          (if (< (.-length buffer) body-end)
            no-message
            (let [body (.toString buffer "utf8" body-start body-end)]
              (swap! state assoc
                     :mode "header"
                     :buffer (remaining-buffer buffer body-end))
              (js/JSON.parse body))))))))

(defn parse-stdio-message!
  "Parse one JSON-RPC message from state. Supports both newline JSON and Content-Length framing."
  [state]
  (let [buffer (:buffer @state)
        mode (:mode @state)
        prefix (when buffer
                 (.toLowerCase (.toString buffer "utf8" 0 (min 32 (.-length buffer)))))]
    (cond
      (nil? buffer) no-message
      (= mode "header") (parse-header-message! state)
      (= mode "line") (parse-line-message! state)
      (first-header-separator buffer) (parse-header-message! state)
      (str/starts-with? prefix "content-length:") no-message
      (not= -1 (.indexOf buffer "\n")) (parse-line-message! state)
      :else no-message)))

(defn- create-dual-stdio-transport []
  (let [transport #js {}
        stdin (.-stdin js/process)
        stdout (.-stdout js/process)
        state (atom {:buffer nil
                     :mode nil
                     :started false
                     :closing false
                     :stdin_fd nil
                     :stdin_fd_owned false
                     :stdin_poller nil
                     :using_stream false})]
    (letfn [(emit-error [err]
              (if-let [handler (.-onerror transport)]
                (handler err)
                (js/console.error "MCP stdio transport error:" (.-message err))))
            (emit-message [message]
              (if-let [handler (.-onmessage transport)]
                (do
                  (stdio-debug "stdio emit message" (or (aget message "method") (aget message "id")))
                  (js/setTimeout (fn [] (handler message)) 0))
                (stdio-debug "stdio missing onmessage handler")))
            (emit-close []
              (when-let [handler (.-onclose transport)]
                (handler)))
            (process-buffer []
              (loop []
                (let [message (try
                                (parse-stdio-message! state)
                                (catch :default err
                                  (emit-error err)
                                  no-message))]
                  (cond
                    (= message no-message) nil
                    (= message skip-message) (recur)
                    :else (do
                            (emit-message message)
                            (recur))))))
            (on-data [chunk]
              (swap! state update :buffer concat-buffer chunk)
              (process-buffer))
            (on-error [err]
              (emit-error err))
            (close-from-stdin []
              (when-not (:closing @state)
                (swap! state assoc :closing true)
                (.close transport)))
            (open-polled-stdin []
              (when-not (= "win32" (.-platform js/process))
                (when (poll-runtime)
                  (stdio-debug "stdio fd polling enabled" 0)
                  0)))
            (poll-stdin-fd []
              (when-let [fd (:stdin_fd @state)]
                (when (fd-readable? fd)
                  (let [buffer (.alloc Buffer 65536)
                        read-result (try
                                      {:n (.readSync fs fd buffer 0 (.-length buffer) nil)}
                                      (catch :default err
                                        {:err err}))]
                    (cond
                      (:err read-result)
                      (emit-error (:err read-result))

                      (pos? (:n read-result))
                      (do
                        (stdio-debug "stdio read bytes" (:n read-result))
                        (swap! state update :buffer concat-buffer (.from Buffer (.subarray buffer 0 (:n read-result))))
                        (process-buffer))

                      (zero? (:n read-result))
                      (do
                        (when-let [poller (:stdin_poller @state)]
                          (js/clearInterval poller))
                        (swap! state assoc :stdin_poller nil)
                        (close-from-stdin))

                      :else nil)))))
            (start-stream-stdin []
              (stdio-debug "stdio stream fallback enabled")
              (swap! state assoc :using_stream true)
              (.on stdin "data" on-data)
              (.on stdin "error" on-error)
              (.on stdin "end" close-from-stdin)
              (.on stdin "close" close-from-stdin)
              (when (.-resume stdin)
                (.resume stdin)))
            (start-polled-stdin []
              (if-let [fd (when (stdio-poll-enabled?)
                            (open-polled-stdin))]
                (let [poller (js/setInterval poll-stdin-fd 10)]
                  (swap! state assoc
                         :stdin_fd fd
                         :stdin_fd_owned false
                         :stdin_poller poller))
                (start-stream-stdin)))]
      (set! (.-start transport)
            (fn []
              (if (:started @state)
                (js/Promise.reject (js/Error. "Stdio transport already started"))
                (do
                  (swap! state assoc :started true)
                  (start-polled-stdin)
                  (js/Promise.resolve)))))
      (set! (.-send transport)
            (fn [message]
              (js/Promise.
               (fn [resolve _reject]
                 (let [payload (serialize-stdio-message message (:mode @state))]
                   (stdio-debug "stdio send bytes" (.byteLength Buffer payload "utf8") "mode" (:mode @state))
                   (if (.write stdout payload)
                     (resolve)
                     (.once stdout "drain" resolve)))))))
      (set! (.-close transport)
            (fn []
              (when (:using_stream @state)
                (.off stdin "data" on-data)
                (.off stdin "error" on-error)
                (.off stdin "end" close-from-stdin)
                (.off stdin "close" close-from-stdin)
                (when (zero? (.listenerCount stdin "data"))
                  (.pause stdin)))
              (when-let [poller (:stdin_poller @state)]
                (js/clearInterval poller))
              (when (and (:stdin_fd_owned @state) (:stdin_fd @state))
                (try
                  (.closeSync fs (:stdin_fd @state))
                  (catch :default _ nil)))
              (swap! state assoc
                     :buffer nil
                     :started false
                     :closing true
                     :stdin_fd nil
                     :stdin_fd_owned false
                     :stdin_poller nil
                     :using_stream false)
              (emit-close)
              (js/Promise.resolve)))
      transport)))

(defn- create-server []
  (let [^js sdk (js/require "@modelcontextprotocol/sdk/server/index.js")
        ^js types (js/require "@modelcontextprotocol/sdk/types.js")
        Server (.-Server sdk)
        ListToolsRequestSchema (.-ListToolsRequestSchema types)
        CallToolRequestSchema (.-CallToolRequestSchema types)
        ListResourcesRequestSchema (.-ListResourcesRequestSchema types)
        ReadResourceRequestSchema (.-ReadResourceRequestSchema types)
        ListPromptsRequestSchema (.-ListPromptsRequestSchema types)
        GetPromptRequestSchema (.-GetPromptRequestSchema types)
        server (new Server #js {:name "fighorse"
                                :version "0.1.0"}
                       #js {:capabilities #js {:tools #js {}
                                               :resources #js {}
                                               :prompts #js {}}})]
    ;; Register tool list handler
    (.setRequestHandler server ListToolsRequestSchema
                        (fn [_request] (tools/list-tools)))
    ;; Register tool call handler
    (.setRequestHandler server CallToolRequestSchema
                        (fn [request] (tools/call-tool request)))
    (when ListResourcesRequestSchema
      (.setRequestHandler server ListResourcesRequestSchema
                          (fn [_request] (clj->js (resources/list-resources)))))
    (when ReadResourceRequestSchema
      (.setRequestHandler server ReadResourceRequestSchema
                          (fn [^js request]
                            (let [^js params (.-params request)
                                  uri (.-uri params)]
                              (clj->js (resources/read-resource uri))))))
    (when ListPromptsRequestSchema
      (.setRequestHandler server ListPromptsRequestSchema
                          (fn [_request] (clj->js (resources/list-prompts)))))
    (when GetPromptRequestSchema
      (.setRequestHandler server GetPromptRequestSchema
                          (fn [^js request]
                            (let [^js params (.-params request)
                                  name (.-name params)
                                  arguments (js->clj (.-arguments params) :keywordize-keys true)]
                              (clj->js (resources/get-prompt name arguments))))))
    server))

(defn- jsonrpc-result [id result]
  #js {:jsonrpc "2.0"
       :id id
       :result result})

(defn- jsonrpc-error [id code message]
  #js {:jsonrpc "2.0"
       :id id
       :error #js {:code code
                   :message message}})

(defn- stdio-initialize-result [^js request]
  (let [^js params (.-params request)
        requested-version (.-protocolVersion params)]
    #js {:protocolVersion (or requested-version "2025-11-25")
         :capabilities #js {:tools #js {}}
         :serverInfo #js {:name "fighorse"
                          :version "0.1.0"}
         :instructions "Call discover_fighorse first. For Figma replication, ask when platform or asset_format is missing, export assets with manifests, and record reusable lessons after visual fixes."}))

(defn- serve-stdio []
  (let [transport (create-dual-stdio-transport)]
    (set! (.-onmessage transport)
          (fn [message]
            (let [id (.-id message)
                  method (.-method message)]
              (case method
                "initialize"
                (.send transport (jsonrpc-result id (stdio-initialize-result message)))

                "ping"
                (.send transport (jsonrpc-result id #js {}))

                "tools/list"
                (.send transport (jsonrpc-result id (tools/list-tools)))

                "tools/call"
                (-> (js/Promise.resolve (tools/call-tool message))
                    (.then (fn [result]
                             (.send transport (jsonrpc-result id result))))
                    (.catch (fn [err]
                              (.send transport (jsonrpc-error id -32603 (or (.-message err) "Internal error"))))))

                "notifications/initialized"
                nil

                "notifications/cancelled"
                nil

                (when-not (nil? id)
                  (.send transport (jsonrpc-error id -32601 (str "Method not found: " method))))))))
    (set! (.-onerror transport)
          (fn [err]
            (js/console.error "MCP stdio error:" (.-message err))))
    (set! (.-onclose transport)
          (fn []
            (cleanup!)
            (js/process.exit 0)))
    (-> (.start transport)
        (.then (fn []
                 (when (= "1" (.-FIGHORSE_MCP_STDIO_LOG js/process.env))
                   (js/console.error "Fighorse MCP server started on stdio"))))
        (.catch (fn [err]
                  (js/console.error "MCP server error:" (.-message err))
                  (js/process.exit 1))))
    transport))

(defn- send-text [^js res status text]
  (.writeHead res status #js {"Content-Type" "text/plain"})
  (.end res text))

(defn- send-json [^js res status data & {:keys [cors-origin]}]
  (.writeHead res status #js {"Content-Type" "application/json"
                              "Access-Control-Allow-Origin" cors-origin})
  (.end res (js/JSON.stringify (clj->js data) nil 2)))

(defn handle-streamable-http-request!
  "Handle one stateless Streamable HTTP MCP request.

   A fresh transport and MCP server are created for every request. This is
   required for clients such as Codex that repeat initialize handshakes."
  [^js req ^js res transport-factory server-factory cors-origin]
  (let [^js transport (transport-factory)
        ^js server (server-factory)
        closed (atom false)
        close-once (fn []
                     (when (compare-and-set! closed false true)
                       (try
                         (.close transport)
                         (catch :default _ nil))
                       (try
                         (.close server)
                         (catch :default _ nil))))]
    (.once res "close" close-once)
    (-> (.connect server transport)
        (.then (fn []
                 (.handleRequest transport req res)))
        (.catch (fn [err]
                  (close-once)
                  (js/console.error "MCP streamable HTTP error:" (.-message err))
                  (when-not (.-headersSent res)
                    (send-json res 500 {:jsonrpc "2.0"
                                        :id nil
                                        :error {:code -32603
                                                :message "MCP streamable HTTP error"}}
                               :cors-origin cors-origin)))))))

(defn- serve-sse [port host cors-origin]
  (let [port (if (number? port) port (js/parseInt port 10))
        host (or host "127.0.0.1")
        cors-origin (or cors-origin (str "http://" host ":" port))
        http (js/require "http")
        ^js sse (js/require "@modelcontextprotocol/sdk/server/sse.js")
        ^js streamable (js/require "@modelcontextprotocol/sdk/server/streamableHttp.js")
        SSEServerTransport (.-SSEServerTransport sse)
        StreamableHTTPServerTransport (.-StreamableHTTPServerTransport streamable)
        transports (atom {})
        handler (fn [^js req ^js res]
                  (let [url (js/URL. (or (.-url req) "/") "http://localhost")
                        pathname (.-pathname url)
                        method (.-method req)]
                    (cond
                      (and (= method "GET") (contains? #{"/" "/manifest" "/discover"} pathname))
                      (send-json res 200 (assoc (discovery/manifest)
                                                :service {:transport "sse"
                                                          :endpoint (str "http://" host ":" port "/sse")
                                                          :streamable_http (str "http://" host ":" port "/mcp")
                                                          :health (str "http://" host ":" port "/health")})
                                 :cors-origin cors-origin)

                      (and (= method "GET") (= pathname "/health"))
                      (send-json res 200 (discovery/doctor) :cors-origin cors-origin)

                      (and (= method "GET") (= pathname "/sse"))
                      (let [^js transport (new SSEServerTransport "/messages" res)
                            session-id (.-sessionId transport)
                            server (create-server)]
                        (swap! transports assoc session-id transport)
                        (set! (.-onclose transport)
                              (fn [] (swap! transports dissoc session-id)))
                        (-> (.connect server transport)
                            (.then (fn []
                                     (js/console.error (str "Fighorse MCP SSE session started: " session-id))))
                            (.catch (fn [err]
                                      (swap! transports dissoc session-id)
                                      (js/console.error "MCP SSE error:" (.-message err))
                                      (when-not (.-headersSent res)
                                        (send-text res 500 "MCP SSE error"))))))

                      (and (= method "POST") (= pathname "/messages"))
                      (let [session-id (.get (.-searchParams url) "sessionId")
                            ^js transport (get @transports session-id)]
                        (if transport
                          (-> (.handlePostMessage transport req res)
                              (.catch (fn [err]
                                        (js/console.error "MCP message error:" (.-message err))
                                        (when-not (.-headersSent res)
                                          (send-text res 500 "MCP message error")))))
                          (send-text res 404 "MCP session not found")))

                      (= pathname "/mcp")
                      (handle-streamable-http-request! req res
                                                       (fn [] (new StreamableHTTPServerTransport #js {:sessionIdGenerator js/undefined}))
                                                       create-server
                                                       cors-origin)

                      :else
                      (send-text res 404 "Not found"))))
        server (.createServer http handler)]
    (.on server "error"
         (fn [err]
           (js/console.error "Fighorse MCP server error:" (or (.-message err) (str err)))
           (js/process.exit 1)))
    (.listen server port host
             (fn []
               (js/console.error (str "Fighorse MCP server listening on http://" host ":" port "/sse and /mcp"))))
    (set! (.-fighorseClose server)
          (fn []
            (doseq [[_ transport] @transports]
              (try
                (.close ^js transport)
                (catch :default _ nil)))
            (reset! transports {})
            (js/Promise.
             (fn [resolve _reject]
               (.close server resolve)))))
    server))

(defn serve
  "Start the MCP server. Defaults to SSE; pass :transport \"stdio\" for subprocess mode.
   The SSE server also exposes Streamable HTTP at /mcp for clients that can reuse
   the installed singleton service."
  [& {:keys [transport port host cors-origin]
      :or {transport "sse" port 9449 host "127.0.0.1"}}]
  (let [transport (or transport "sse")
        lock (try
               (acquire-singleton-lock! transport port)
               (catch :default err
                 (js/console.error (or (.-message err) (str err)))
                 (js/process.exit 2)))]
    (try
      (let [^js resource (case transport
                           "stdio" (serve-stdio)
                           "sse" (serve-sse port host cors-origin)
                           "http" (serve-sse port host cors-origin)
                           (do
                             (js/console.error (str "Unknown MCP transport: " transport))
                             (release-singleton-lock! lock)
                             (js/process.exit 1)))
            close-fn (fn []
                       (if-let [close (.-fighorseClose resource)]
                         (close)
                         (.close resource)))]
        (install-shutdown-handlers! lock close-fn)
        resource)
      (catch :default err
        (release-singleton-lock! lock)
        (throw err)))))

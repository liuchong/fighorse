(ns fighorse.install-test
  (:require [cljs.test :refer [deftest is testing]]
            [clojure.string :as str]
            [fighorse.experience :as experience]
            [fighorse.install :as install]))

(def ^:private fs (js/require "fs"))
(def ^:private os (js/require "os"))
(def ^:private path (js/require "path"))

(defn- with-temp-env [f]
  (let [original-home (.-FIGHORSE_HOME js/process.env)
        original-path (.-FIGHORSE_EXPERIENCE_PATH js/process.env)
        original-scope (.-FIGHORSE_EXPERIENCE_SCOPE js/process.env)
        original-project (.-FIGHORSE_PROJECT_DIR js/process.env)
        home (.mkdtempSync fs (.join path (.tmpdir os) "fighorse-home-"))
        project (.mkdtempSync fs (.join path (.tmpdir os) "fighorse-project-"))]
    (set! (.-FIGHORSE_HOME js/process.env) home)
    (js-delete js/process.env "FIGHORSE_EXPERIENCE_PATH")
    (js-delete js/process.env "FIGHORSE_EXPERIENCE_SCOPE")
    (js-delete js/process.env "FIGHORSE_PROJECT_DIR")
    (try
      (f home project)
      (finally
        (if (nil? original-home)
          (js-delete js/process.env "FIGHORSE_HOME")
          (set! (.-FIGHORSE_HOME js/process.env) original-home))
        (if (nil? original-path)
          (js-delete js/process.env "FIGHORSE_EXPERIENCE_PATH")
          (set! (.-FIGHORSE_EXPERIENCE_PATH js/process.env) original-path))
        (if (nil? original-scope)
          (js-delete js/process.env "FIGHORSE_EXPERIENCE_SCOPE")
          (set! (.-FIGHORSE_EXPERIENCE_SCOPE js/process.env) original-scope))
        (if (nil? original-project)
          (js-delete js/process.env "FIGHORSE_PROJECT_DIR")
          (set! (.-FIGHORSE_PROJECT_DIR js/process.env) original-project))))))

(deftest install-home-client-service-and-skill
  (testing "installer creates reviewable local artifacts"
    (with-temp-env
      (fn [home _project]
        (let [home-result (install/install-home!)
              auth-dry (install/install-auth!)
              auth-result (install/install-auth! :token "figd_test_token" :apply true)
              client-result (install/install-client! :client "codex")
              kimi-result (install/install-client! :client "kimi-cli")
              binary-result (install/install-binary! :source "dist/fighorse")
              service-result (install/install-service! :service "systemd" :port 9559)
              skill-result (install/install-skill!)]
          (is (= "fighorse.install-home.v1" (:kind home-result)))
          (is (= "fighorse.ai-contract.v1" (get-in home-result [:ai_contract :kind])))
          (is (.existsSync fs (.join path home "bin")))
          (is (.existsSync fs (.join path home "experience")))
          (is (= false (:apply auth-dry)))
          (is (= true (:ok auth-result)))
          (is (= "figd_t..." (:token_mask auth-result)))
          (is (not (str/includes? (pr-str auth-result) "figd_test_token")))
          (is (.existsSync fs (.join path home "config.json")))
          (is (= false (:apply binary-result)))
          (is (= (.join path home "bin" "fighorse") (:target binary-result)))
          (is (= "codex" (:client client-result)))
          (is (some #(str/ends-with? % "codex-config.toml") (:files client-result)))
          (is (.existsSync fs (.join path home "clients" "codex" "mcp.json")))
          (is (str/includes? (.readFileSync fs (.join path home "clients" "codex" "codex-config.toml") "utf8")
                             "url = \"http://127.0.0.1:9449/mcp\""))
          (is (str/includes? (.readFileSync fs (.join path home "clients" "codex" "codex-config.toml") "utf8")
                             "startup_timeout_sec = 60"))
          (is (= "http" (get-in (js->clj (js/JSON.parse (.readFileSync fs (.join path home "clients" "codex" "mcp.json") "utf8"))
                                         :keywordize-keys true)
                                [:mcpServers :fighorse :transport])))
          (is (= "kimi" (:client kimi-result)))
          (is (some #(str/ends-with? % "KIMI.md") (:files kimi-result)))
          (is (.existsSync fs (.join path home "clients" "kimi" "mcp.json")))
          (is (= false (:apply kimi-result)))
          (is (str/includes? (get-in (install/client-detection "cursor") [:skill_dir])
                             "/.cursor/skills/fighorse"))
          (is (not (str/includes? (get-in (install/client-detection "cursor") [:skill_dir])
                                  "skills-cursor")))
          (is (= "systemd" (:service service-result)))
          (is (= "http+sse" (:transport service-result)))
          (is (str/includes? (.readFileSync fs (:file service-result) "utf8")
                             "ExecStart=fighorse mcp serve --transport sse --host 127.0.0.1"))
          (is (.existsSync fs (.join path home "skills" "fighorse" "SKILL.md")))
          (is (str/includes? (.readFileSync fs (first (:files skill-result)) "utf8")
                             "name: fighorse"))
          (is (str/includes? (.readFileSync fs (first (:files skill-result)) "utf8")
                             "description: Recreate, inspect, export, and debug Figma designs"))
          (is (str/includes? (.readFileSync fs (first (:files skill-result)) "utf8")
                             "record_experience"))
          (is (str/includes? (.readFileSync fs (first (:files skill-result)) "utf8")
                             "fighorse auth login --token <FIGMA_TOKEN>"))
          (is (str/includes? (.readFileSync fs (first (:files skill-result)) "utf8")
                             "auth.has_token"))
          (is (str/includes? (.readFileSync fs (first (:files skill-result)) "utf8")
                             "figma-api coverage"))
          (is (str/includes? (.readFileSync fs (first (:files skill-result)) "utf8")
                             "visual_audit"))
          (is (str/includes? (.readFileSync fs (first (:files skill-result)) "utf8")
                             ".fighorse/exports"))
          (let [manifest (js->clj (js/JSON.parse (.readFileSync fs (.join path home "clients" "codex" "fighorse-client.json") "utf8"))
                                  :keywordize-keys true)]
            (is (some #(= "visual_audit" %)
                      (:recommended_tool_order manifest)))
            (is (some #(str/includes? % "fighorse://coverage")
                      (:notes manifest))))
          (is (= "fighorse.ai-contract.v1" (get-in client-result [:ai_contract :kind]))))))))

(deftest project-install-enables-project-experience
  (testing "project installs switch auto scope to project while reading global fallback"
    (with-temp-env
      (fn [home project]
        (experience/add! {:summary "Global lesson"
                          :lesson "Global lessons apply before project-specific lessons."}
                         {:scope "global"
                          :project-dir project})
        (is (= (.join path home "experience" "global.jsonl")
               (experience/experience-path :project-dir project)))
        (let [installed (install/install-project! :project-dir project)]
          (is (= "fighorse.install-project.v1" (:kind installed)))
          (is (= "fighorse.ai-contract.v1" (get-in installed [:ai_contract :kind])))
          (is (.existsSync fs (.join path project ".fighorse" "fighorse.json")))
          (let [project-config (js->clj (js/JSON.parse (.readFileSync fs (.join path project ".fighorse" "fighorse.json") "utf8"))
                                        :keywordize-keys true)]
            (is (= ".fighorse/exports" (get-in project-config [:exports :scratch])))
            (is (= true (get-in project-config [:exports :manifest_required]))))
          (is (= (.join path project ".fighorse" "experience.jsonl")
                 (experience/experience-path :project-dir project)))
          (experience/add! {:summary "Project lesson"
                            :lesson "Project lessons should be read before global lessons."}
                           {:project-dir project})
          (let [summary (experience/list-experiences :project-dir project)]
            (is (= 2 (:total_count summary)))
            (is (= "Project lesson" (get-in summary [:records 0 :summary])))
            (is (= "Global lesson" (get-in summary [:records 1 :summary])))))))))

(deftest cli-mode-install-does-not-configure-or-start-mcp-service
  (testing "CLI-only install path avoids ports, service managers, and MCP clients"
    (with-temp-env
      (fn [_home _project]
        (let [installed (install/install-all!)]
          (is (= "cli" (:mode installed)))
          (is (empty? (:clients installed)))
          (is (empty? (:clients_result installed)))
          (is (= "none" (get-in installed [:service :service])))
          (is (= true (get-in installed [:service :skipped])))))))
  (testing "long-running MCP service mode must be explicit"
    (with-temp-env
      (fn [_home _project]
        (let [installed (install/install-all! :mode "service" :clients "cursor,codex")]
          (is (= "service" (:mode installed)))
          (is (= ["cursor" "codex"] (:clients installed)))
          (is (= "http+sse" (get-in installed [:service :transport])))
          (is (some? (get-in installed [:service :file]))))))))

(deftest install-status-explains-public-onboarding-and-service-mode
  (testing "status makes CLI-only and explicit service paths visible"
    (with-temp-env
      (fn [_home _project]
        (let [status (install/status)]
          (is (= "cli" (get-in status [:public_quickstart :default_install_mode])))
          (is (clojure.string/includes? (get-in status [:public_quickstart :service_install])
                                        "--mode service"))
          (is (= "http://127.0.0.1:9449/mcp"
                 (get-in status [:mcp_service :endpoint])))
          (is (contains? (:troubleshooting status) :codex_handshake)))))))

(deftest package-scripts-cover-source-build-package-and-install
  (testing "package.json exposes source-state package and install workflows"
    (let [pkg (js->clj (js/JSON.parse (.readFileSync fs "package.json" "utf8"))
                       :keywordize-keys true)
          scripts (:scripts pkg)]
      (is (str/includes? (:package scripts) "package:macos"))
      (is (str/includes? (get scripts (keyword "package:darwin-x64")) "bun run build"))
      (is (str/includes? (get scripts (keyword "package:darwin-arm64")) "--target=bun-darwin-arm64"))
      (is (str/includes? (get scripts (keyword "install:local")) "fighorse install --default --apply")))))

(deftest self-install-plans-and-can-copy-current-binary
  (testing "self install supports default/path guidance and explicit apply"
    (with-temp-env
      (fn [home _project]
        (let [source (.join path home "source-fighorse")
              install-dir (.join path home "target-bin")]
          (.writeFileSync fs source "#!/bin/sh\necho fighorse\n")
          (.chmodSync fs source 493)
          (let [dry (install/install-self! :source source
                                           :home home
                                           :path install-dir
                                           :link-dirs "none")
                installed (install/install-self! :source source
                                                 :home home
                                                 :path install-dir
                                                 :link-dirs "none"
                                                 :apply true)]
            (is (= "fighorse.install-self.v1" (:kind dry)))
            (is (= "fighorse.install-guide.v1" (get-in dry [:guide :kind])))
            (is (str/includes? (get-in dry [:guide :default_install :command])
                               "--default --apply"))
            (is (= (.join path install-dir "fighorse")
                   (get-in dry [:binary :target])))
            (is (.existsSync fs (.join path install-dir "fighorse")))
            (is (= [] (get-in installed [:binary :links])))
            (is (.existsSync fs (.join path home "skills" "fighorse" "SKILL.md")))))))))

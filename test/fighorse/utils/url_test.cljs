(ns fighorse.utils.url-test
  (:require [cljs.test :refer [deftest is testing]]
            [fighorse.utils.url :as url]))

(deftest build-query-test
  (testing "empty params returns nil"
    (is (nil? (url/build-query {}))))
  (testing "single param"
    (is (= "?depth=2" (url/build-query {:depth 2}))))
  (testing "multiple params"
    (is (= "?depth=2&version=123" (url/build-query {:depth 2 :version "123"}))))
  (testing "nil values are omitted"
    (is (= "?depth=2" (url/build-query {:depth 2 :version nil}))))
  (testing "special characters are encoded"
    (is (= "?ids=1%3A2%2C1%3A3" (url/build-query {:ids "1:2,1:3"})))))

(deftest build-url-test
  (testing "URL without params"
    (is (= "https://api.figma.com/v1/files/abc" (url/build-url "https://api.figma.com/v1/files/abc" {}))))
  (testing "URL with params"
    (is (= "https://api.figma.com/v1/files/abc?depth=2"
           (url/build-url "https://api.figma.com/v1/files/abc" {:depth 2})))))

(deftest parse-figma-url-test
  (testing "parses design URL and normalizes node id"
    (is (= {:valid true
            :input "https://www.figma.com/design/SApEHB4JyKN2I8PpMYfgbK/Intent?node-id=376-5412"
            :kind "design"
            :file_key "SApEHB4JyKN2I8PpMYfgbK"
            :raw_node_id "376-5412"
            :node_id "376:5412"}
           (url/parse-figma-url "https://www.figma.com/design/SApEHB4JyKN2I8PpMYfgbK/Intent?node-id=376-5412"))))
  (testing "accepts raw file key"
    (is (= {:valid true
            :input "abc123"
            :file_key "abc123"
            :kind "file_key"}
           (url/parse-figma-url "abc123"))))
  (testing "reports invalid URL"
    (is (= false (:valid (url/parse-figma-url "https://example.com/no-file-key"))))))

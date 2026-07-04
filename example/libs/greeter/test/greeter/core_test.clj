(ns greeter.core-test
  (:require [clojure.test :refer [deftest is]]
            [greeter.core :as core]))

(deftest greet-test
  (is (= "Hello, moon!" (core/greet "moon"))))

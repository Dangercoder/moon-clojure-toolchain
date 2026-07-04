(ns cli.main
  (:require [greeter.core :as greeter])
  (:gen-class))

(defn -main [& args]
  (println (greeter/greet (or (first args) "world"))))

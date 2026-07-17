(ns cli.main
  (:require [greeter.core :as greeter]))

(defn -main [& args]
  (println (greeter/greet (first args))))

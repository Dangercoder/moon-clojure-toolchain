(ns greeter.core)

(defn greet [name]
  (str "Hello, " (or name "world") "!"))

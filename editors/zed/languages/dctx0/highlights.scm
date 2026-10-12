(comment) @comment
(string) @string
(triple_string) @string
(number) @number
(declaration_keyword) @keyword
(keyword) @keyword
(builtin_type) @type.builtin
(operator) @operator

(declaration
  kind: (declaration_keyword) @_kind
  name: (identifier) @function
  (#eq? @_kind "operation"))

(declaration
  name: (identifier) @type)

(identifier) @boolean
  (#any-of? @boolean "true" "false")
(identifier) @keyword
  (#any-of? @keyword "is" "and" "or" "not" "implies" "in" "for" "uses" "as" "result")
(identifier) @function.builtin
  (#any-of? @function.builtin "count" "sum" "all" "any" "contains" "distinct" "min" "max" "filter" "map" "sortBy" "sortDescBy" "first" "single" "ifElse" "mean" "indexOf" "getOrElse" "lowercase" "uppercase" "trim" "length" "startsWith" "endsWith" "includesText" "slice" "reverse" "divide" "round")

(arguments (declaration_keyword) @variable
  (#eq? @variable "value"))

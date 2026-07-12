(tag_name) @tag

((tag_name) @tag.builtin
  (#match? @tag.builtin "^[A-Za-z][A-Za-z0-9-]*:"))

(doctype) @tag.doctype
(xml_declaration) @tag.doctype
(attribute_name) @attribute
(quoted_attribute_value) @string
(attribute_text) @string
(comment) @comment
(entity) @string.special

"=" @punctuation.delimiter.html

[
  "<"
  ">"
  "<!"
  "</"
  "/>"
] @punctuation.bracket.html

[
  (expression_start)
  (expression_end)
] @punctuation.special

(global_identifier) @variable.builtin
(identifier) @variable

(member_expression
  property: (identifier) @property)

(call_expression
  function: (identifier) @function)

(call_expression
  function: (member_expression
    property: (identifier) @function.method))

(operator) @operator
(string_literal) @string
(number_literal) @number
(boolean_literal) @boolean
(null_literal) @constant.builtin

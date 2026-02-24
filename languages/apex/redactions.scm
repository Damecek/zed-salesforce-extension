; Redactions for Salesforce Apex

; Redact number literals
(int) @redact
(decimal_floating_point_literal) @redact

; Redact string literals
(string_literal) @redact

; Redact sensitive variables (e.g., passwords, tokens)
(variable_declarator
  name: (identifier) @_name
  (#match? @_name "(password|token|secret|key)")
  value: (_) @redact)

; Redact sensitive method calls (e.g., authentication methods)
(method_invocation
  name: (identifier) @_method
  (#match? @_method "(authenticate|login|getToken)")
  arguments: (argument_list (_) @redact))

; Redact sensitive field declarations
(field_declaration
  declarator: (variable_declarator
    name: (identifier) @_field
    (#match? @_field "(apiKey|clientSecret|accessToken)")
    value: (_) @redact))

; Redact sensitive annotations
(annotation
  name: (identifier) @_annotation
  (#match? @_annotation "Sensitive")
  arguments: (annotation_argument_list (_) @redact))

;; Apex (Salesforce) highlights for Zed

;------------------------------------------------------------------------------
; Comments
;------------------------------------------------------------------------------

[
  (line_comment)
  (block_comment)
] @comment

;------------------------------------------------------------------------------
; Literals
;------------------------------------------------------------------------------

(string_literal) @string

[
  (int)
  (decimal_floating_point_literal)
] @number

(boolean) @boolean
(null_literal) @constant

;------------------------------------------------------------------------------
; Punctuation / delimiters / brackets
;------------------------------------------------------------------------------

"@" @punctuation.special

[
  "."
  ","
  ";"
  ":"
] @punctuation.delimiter

[
  "("
  ")"
  "{"
  "}"
  "["
  "]"
] @punctuation.bracket

;------------------------------------------------------------------------------
; Operators
;------------------------------------------------------------------------------

(assignment_operator) @operator

(update_operator) @operator
(safe_navigation_operator) @operator

(binary_expression
  operator: [
    ">"
    "<"
    ">="
    "<="
    "=="
    "==="
    "!="
    "!=="
    "&&"
    "||"
    "+"
    "-"
    "*"
    "/"
    "&"
    "|"
    "^"
    "%"
    "<<"
    ">>"
    ">>>"
  ] @operator)

(unary_expression
  operator: [
    "+"
    "-"
    "!"
    "~"
  ] @operator)

[
  "="
  "=>"
] @operator

;------------------------------------------------------------------------------
; Keywords / modifiers
;------------------------------------------------------------------------------

[
  (all_rows_clause)
  "break"
  "catch"
  "class"
  "continue"
  "do"
  "else"
  "enum"
  "extends"
  "finally"
  "for"
  "get"
  "if"
  "implements"
  "instanceof"
  "interface"
  "new"
  "on"
  "return"
  "set"
  "switch"
  "throw"
  "try"
  "trigger"
  "when"
  "while"
  "public"
  "private"
  "protected"
  "global"
  "static"
  "final"
  "virtual"
  "abstract"
  "override"
  "transient"
  "webservice"
  "testMethod"
] @keyword

; Sharing modifiers
[
  (with_sharing)
  (without_sharing)
  (inherited_sharing)
] @keyword

;------------------------------------------------------------------------------
; Types
;------------------------------------------------------------------------------

(class_declaration
  name: (identifier) @type)

(interface_declaration
  name: (identifier) @type)

(enum_declaration
  name: (identifier) @type)

(class_declaration
  (superclass) @type)

(interfaces
  (type_list
    (type_identifier) @type))

; Type identifiers in common type positions
(type_identifier) @type
(scoped_type_identifier (type_identifier) @type)

(generic_type
  (type_identifier) @type)

(type_arguments
  "<" @punctuation.bracket
  ">" @punctuation.bracket
  (type_identifier) @type)

(array_type
  element: (type_identifier) @type)

(object_creation_expression
  type: (type_identifier) @type)

(array_creation_expression
  type: (type_identifier) @type)

[
  (boolean_type)
  (void_type)
] @type.builtin

;------------------------------------------------------------------------------
; Enums
;------------------------------------------------------------------------------

(enum_constant
  name: (identifier) @constant)

;------------------------------------------------------------------------------
; Constants
;------------------------------------------------------------------------------

; Screaming snake case convention (e.g., MY_CONSTANT, MAX_SIZE)
((identifier) @constant
  (#match? @constant "^_*[A-Z][A-Z\\d_]+$"))

; Static final fields are constants
(field_declaration
  (modifiers (modifier [(final) (static)])(modifier [(final) (static)]))
  (variable_declarator
    name: (identifier) @constant))

; Switch when labels
(switch_rule
  (switch_label
    (identifier) @constant))

;------------------------------------------------------------------------------
; Functions / methods
;------------------------------------------------------------------------------

(method_declaration
  name: (identifier) @function)

(method_invocation
  name: (identifier) @function)

; Constructor declarations
(constructor_declaration
  name: (identifier) @constructor)

; Common builtins
(this) @variable.builtin
(super) @function.builtin

; DML operations (insert, update, delete, upsert, undelete, merge)
(dml_type) @function.builtin

; System.runAs
(method_invocation
  object: (identifier) @_obj
  name: (identifier) @function.builtin
  (#eq? @_obj "System")
  (#eq? @function.builtin "runAs"))

;------------------------------------------------------------------------------
; Variables
;------------------------------------------------------------------------------

; Parameters
(formal_parameter
  name: (identifier) @variable)

(method_declaration
  (formal_parameters
    (formal_parameter
      name: (identifier) @variable.parameter)))

; Local variable declarations
(local_variable_declaration
  declarator: (variable_declarator
    name: (identifier) @variable))

; Field declarations
(field_declaration
  declarator: (variable_declarator
    name: (identifier) @property))

; Enhanced for: `for (Type name : value)`
(enhanced_for_statement
  name: (identifier) @variable)

(enhanced_for_statement
  value: (identifier) @variable)

; LHS assignments (helps highlight variables when declared elsewhere)
(assignment_expression
  left: (identifier) @variable)

; For-loop condition and update variables
(for_statement
  condition: (binary_expression
    (identifier) @variable))

(for_statement
  update: (update_expression
    (identifier) @variable))

; Identifier occurrences in common expression contexts
(argument_list
  (identifier) @variable)

(explicit_constructor_invocation
  arguments: (argument_list
    (identifier) @variable))

(expression_statement (_ (identifier) @variable))

(return_statement
  (identifier) @variable)

(bound_apex_expression
  (identifier) @variable)

(instanceof_expression
  left: (identifier) @variable
  right: (type_identifier) @type)

(cast_expression
  type: (type_identifier) @type
  value: (identifier) @variable)

(binary_expression
  (identifier) @variable)

(switch_expression
  condition: (identifier) @variable)

;------------------------------------------------------------------------------
; Properties / member access
;------------------------------------------------------------------------------

(field_access
  field: (identifier) @property)

; Best-effort: If something looks like a TypeName.member, treat the prefix as a type.
; This is useful for `System.debug`, `Account.SObjectType`, etc.
((scoped_identifier
  scope: (identifier) @type)
 (#match? @type "^[A-Z]"))

((method_invocation
  object: (identifier) @type)
 (#match? @type "^[A-Z]"))

;------------------------------------------------------------------------------
; Annotations
;------------------------------------------------------------------------------

(annotation
  "@" @punctuation.special
  name: (identifier) @keyword
  (#match? @keyword "^(AuraEnabled|Deprecated|Future|InvocableMethod|InvocableVariable|IsTest|isTest|JsonAccess|NamespaceAccessible|ReadOnly|RemoteAction|SuppressWarnings|TestSetup|TestVisible|RestResource|HttpDelete|HttpGet|HttpPatch|HttpPost|HttpPut)$"))

(annotation_key_value
  (identifier) @property)

;------------------------------------------------------------------------------
; DML / Triggers
;------------------------------------------------------------------------------

(trigger_declaration
  name: (identifier) @type
  object: (identifier) @type
  (trigger_event) @keyword
  ("," (trigger_event) @keyword)*)

(when_sobject_type
  (type_identifier) @type
  (identifier) @variable)

;------------------------------------------------------------------------------
; Modifier safety net (belt-and-suspenders)
; Ensures keyword highlighting works even when modifiers follow annotations
;------------------------------------------------------------------------------

(method_declaration (modifiers (modifier) @keyword))
(field_declaration (modifiers (modifier) @keyword))
(constructor_declaration (modifiers (modifier) @keyword))
(class_declaration (modifiers (modifier) @keyword))
(interface_declaration (modifiers (modifier) @keyword))
(enum_declaration (modifiers (modifier) @keyword))

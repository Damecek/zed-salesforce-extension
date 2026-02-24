; ------------------------
; Types
; ------------------------

(class_declaration
  name: (identifier) @name) @item

(interface_declaration
  name: (identifier) @name) @item

(enum_declaration
  name: (identifier) @name) @item

(trigger_declaration
  name: (identifier) @name) @item

; ------------------------
; Members
; ------------------------

(method_declaration
  name: (identifier) @name) @item

(constructor_declaration
  name: (identifier) @name) @item

(field_declaration
  declarator: (variable_declarator
    name: (identifier) @name)) @item

; Class-level: @isTest annotation on class
(
  (class_declaration
    (modifiers
      (annotation
        name: (identifier) @_ann
        (#match? @_ann "(?i)^isTest$")))
    name: (identifier) @run)
  (#set! tag apex-test-class)
)

; Method-level: testMethod modifier
(
  (class_declaration
    (class_body
      (method_declaration
        (modifiers
          (modifier
            (testMethod)))
        name: (identifier) @run)))
  (#set! tag apex-test)
)

; Method-level: @isTest annotation on method
(
  (class_declaration
    (class_body
      (method_declaration
        (modifiers
          (annotation
            name: (identifier) @_ann
            (#match? @_ann "(?i)^isTest$")))
        name: (identifier) @run)))
  (#set! tag apex-test)
)

(class_declaration
  (class_body
    (method_declaration
      (modifiers
        (modifier
          (testMethod))) @_mod
      name: (identifier) @run @test_method)))
(#set! tag apex-test)

(class_declaration
  (class_body
    (method_declaration
      (modifiers
        (annotation
          name: (identifier) @_ann
          (#eq? @_ann "isTest")))
      name: (identifier) @run @test_method)))
(#set! tag apex-test)

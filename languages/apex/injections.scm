;; Inject SOQL and SOSL highlighting into Apex files.
;;
;; The Apex grammar (from aheber/tree-sitter-sfapex) represents inline queries
;; with a `query_expression` node whose child is `soql_query_body` or
;; `sosl_query_body`.

;------------------------------------------------------------------------------
; SOQL injection
;------------------------------------------------------------------------------

(query_expression
  (soql_query_body) @injection.content
  (#set! injection.language "soql"))

;------------------------------------------------------------------------------
; SOSL injection
;------------------------------------------------------------------------------

(query_expression
  (sosl_query_body) @injection.content
  (#set! injection.language "sosl"))

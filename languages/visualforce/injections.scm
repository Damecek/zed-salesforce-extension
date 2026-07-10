(script_element
  (raw_text) @injection.content
  (#set! injection.language "javascript"))

(style_element
  (raw_text) @injection.content
  (#set! injection.language "css"))

(attribute
  (attribute_name) @_event_attribute
  (quoted_attribute_value
    (attribute_text) @injection.content)
  (#match? @_event_attribute "^[Oo][Nn][A-Za-z]+$")
  (#set! injection.language "javascript"))

(attribute
  (attribute_name) @_style_attribute
  (quoted_attribute_value
    (attribute_text) @injection.content)
  (#eq? @_style_attribute "style")
  (#set! injection.language "css"))

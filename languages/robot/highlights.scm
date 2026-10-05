; Syntax highlighting for Robot Framework.
;
; Capture names follow the ones Zed themes understand (keyword, function,
; variable, string, ...). Control-flow markers are captured on the token
; itself, never on the whole statement node, so block bodies keep their own
; colors.

[
  (comment)
  (extra_text)
] @comment

(section_header) @title

; Settings: `Library`, `Suite Setup`, `[Documentation]`, `[Tags]`, ...
(setting_name) @keyword
(keyword_setting_name) @attribute
(test_case_setting_name) @attribute
(keyword_setting [ "[" "]" ] @punctuation.bracket)
(test_case_setting [ "[" "]" ] @punctuation.bracket)

; Definitions
(keyword_definition (name) @function)
(test_case_definition (name) @function)

; Keyword calls. `BREAK`/`CONTINUE` inside an inline `IF` are parsed as
; keyword calls, so pick them out by name.
((keyword) @keyword
  (#match? @keyword "^(BREAK|CONTINUE)$"))
((keyword) @function.call
  (#not-match? @function.call "^(BREAK|CONTINUE)$"))

; Variables
[
  (scalar_variable)
  (list_variable)
  (dictionary_variable)
  (environment_variable)
] @variable
(variable_assignment (variable_name) @variable)
(variable_assignment [ "${" "}" ] @variable)
(variable_key) @property
(variable_definition [ "=" " =" ] @operator)
(variable_assignment [ "=" " =" ] @operator)

; Values
(text_chunk) @string
(return_value) @string
(inline_python_expression [ "${{" "}}" ] @punctuation.special)

(ellipses) @punctuation.delimiter

; Control flow
[
  "FOR"
  "IN"
  "IN RANGE"
  "IN ENUMERATE"
  "IN ZIP"
  "WHILE"
  "VAR"
  "GROUP"
  "IF"
  "ELSE IF"
  "ELSE"
  "TRY"
  "EXCEPT"
  "FINALLY"
  "END"
  "RETURN"
  (break_statement)
  (continue_statement)
] @keyword

; Common literals inside arguments
((text_chunk) @boolean
  (#match? @boolean "^(?i)(true|false)$"))
((text_chunk) @constant
  (#match? @constant "^(?i)(none|empty)$"))
((text_chunk) @number
  (#match? @number "^-?[0-9]+(\\.[0-9]+)?$"))

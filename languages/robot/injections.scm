; Inline Python evaluation: ${{ len($items) + 1 }}
; Only the outermost expression: nested parentheses are python_expressions too.
((inline_python_expression (python_expression) @injection.content)
  (#set! injection.language "python"))

; IF / ELSE IF / WHILE conditions are Python expressions. Only conditions that
; refer to variables as `$name` (or use none) get Python highlighting: in
; `${name} > 3` Robot Framework substitutes the value before Python sees it,
; and the Python grammar would split the variable apart.
([
  (if_statement condition: (argument) @injection.content)
  (elseif_statement condition: (argument) @injection.content)
  (inline_if_statement condition: (argument) @injection.content)
  (inline_elseif_statement condition: (argument) @injection.content)
  (while_statement condition: (arguments . (argument) @injection.content))
]
  (#not-match? @injection.content "[$@&%]\\{")
  (#set! injection.language "python"))

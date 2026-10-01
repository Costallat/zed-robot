; Inline Python evaluation: ${{ len($items) + 1 }}
((python_expression) @injection.content
  (#set! injection.language "python"))

; Indent the body of test cases, tasks and keywords, and of control blocks.
(keyword_definition) @indent
(test_case_definition) @indent

(for_statement "END" @end) @indent
(while_statement "END" @end) @indent
(if_statement "END" @end) @indent
(try_statement "END" @end) @indent
(group_statement "END" @end) @indent

; Branch markers line up with the statement that opened the block.
(elseif_statement) @outdent
(else_statement) @outdent
(except_statement) @outdent
(finally_statement) @outdent

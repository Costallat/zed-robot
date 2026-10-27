*** Test Cases ***
Escapes
    Log    Line\nbreak    \${not_a_var}    \#not_a_comment    C:\\temp
    #          ^ string.escape
    #                     ^ string.escape
    #                       ^ string
    #                                      ^ string.escape
    #                                                           ^ string.escape
    Log    a\=b    a == b    https://x.com/?q=1
    #      ^ string
    #       ^ string.escape
    #              ^ string
    #                        ^ string

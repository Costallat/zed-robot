*** Settings ***
# <- title
Library    SSHLibrary    timeout=10s
# <- keyword
#          ^ string
#                        ^ property
#                               ^ operator
#                                ^ string
Test Tags    smoke
# <- keyword

*** Variables ***
${GREETING}    Hello
# <- variable
#              ^ string
&{USER}    name=bob    age=3
#          ^ property
#                      ^ property

*** Test Cases ***
Greets The World
# <- function
    [Documentation]    Says hi
    #^ attribute
    #                  ^ string
    ${result} =    Greet    ${GREETING}[0]
    #^ variable
    #              ^ function.call
    #                       ^ variable
    #                                  ^ property
    Log    ${{ len("abc") + 1 }}
    #      ^ punctuation.special
    Given user is logged in
    #^ keyword
    #     ^ function.call
    Log    True    None    42    -1.5
    #      ^ boolean
    #              ^ constant
    #                      ^ number
    #                            ^ number
    Log    %{HOME}    @{ITEMS}
    #      ^ variable
    #                 ^ variable

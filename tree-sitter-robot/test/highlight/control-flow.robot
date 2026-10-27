*** Test Cases ***
Loops And Branches
    FOR    ${item}    IN    @{ITEMS}
    #^ keyword
    #                 ^ keyword
        IF    $item == 'b'    CONTINUE
        #^ keyword
        #                     ^ keyword
    END
    #^ keyword
    FOR    ${i}    IN RANGE    3
    #              ^ keyword
        BREAK
        #^ keyword
    END
    WHILE    True    limit=3
    #^ keyword
    #                ^ property
        No Operation
    END
    TRY
    #^ keyword
        Fail    boom
    EXCEPT    boom
    #^ keyword
        No Operation
    FINALLY
    #^ keyword
        Log    done
    END
    VAR    ${x}    value
    #^ keyword
    GROUP    Named
    #^ keyword
        Log    ${x}
    END

*** Keywords ***
Give Back
# <- function
    [Arguments]    ${value}
    #^ attribute
    RETURN    ${value}
    #^ keyword
    #         ^ variable

# leading comment
*** Settings ***
Documentation     Example suite
Library           Collections
Resource          common.resource
Test Tags         smoke

*** Variables ***
${GREETING}       Hello
@{ITEMS}          a    b    c
&{USER}           name=bob    age=3

*** Test Cases ***
Greets The World
    [Documentation]    Says hi
    [Tags]    one    two
    ${result} =    Greet    ${GREETING}[0]
    Log    ${{ len("abc") + 1 }}
    FOR    ${item}    IN    @{ITEMS}
        IF    $item == 'b'    CONTINUE
        Log    ${item}
    END
    WHILE    True    limit=3
        BREAK
    END
    TRY
        Fail    boom
    EXCEPT    boom
        No Operation
    ELSE
        No Operation
    FINALLY
        Log    done
    END
    IF    ${True}
        Log    yes
    ELSE IF    ${False}
        Log    no
    ELSE
        Log    maybe
    END

*** Keywords ***
Greet
    [Arguments]    ${name}
    [Tags]    kw
    Log    ${name}
    RETURN    ${name}

Greet ${who} Nicely    [Tags]    embedded
    Log    ${who}

New Syntax
    VAR    ${x}    value
    VAR    @{list}    a    b    scope=SUITE
    GROUP    Named group
        Log    ${x}
    END
    VAR    &{USER_INFO}    name=bob
    Log    ${devices.${E7_2_TYPE}.ip}    %{HOME}    ${TIMEZONE_${index}_TZ}

*** Keywords ***
With Setup
    [Setup]    Log    setup
    No Operation

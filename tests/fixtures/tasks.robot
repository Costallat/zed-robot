*** Comments ***
Anything here is ignored by Robot Framework.

*** Settings ***
Resource    keywords.resource

*** Tasks ***
Process Files
    [Tags]    nightly
    Connect    host=server
    FOR    ${file}    IN    @{FILES}
        TRY
            Log    ${file}
        EXCEPT    *Error*    type=glob    AS    ${err}
            Log    ${err}
        END
    END

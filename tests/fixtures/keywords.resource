*** Settings ***
Documentation     Shared keywords
Library           OperatingSystem
Library           SSHLibrary    timeout=10s    prompt=$
Variables         settings.py

*** Variables ***
${HOST}           example.com
&{OPTIONS}        retries=3    verbose=${True}

*** Keywords ***
Connect
    [Documentation]    Opens a connection.\nSecond line.
    [Arguments]    ${host}=${HOST}    ${port}=22
    Open Connection    ${host}    port=${port}    alias=main
    Log    Path is C:\\temp, not a \${variable} and \# not a comment
    IF    $port > 1024 and $host != 'localhost'
        Log    unprivileged    level=DEBUG
    ELSE IF    ${port} == 22
        Log    ssh
    END
    WHILE    $port < 30    limit=5
        BREAK
    END
    IF    $host    RETURN    ${host}

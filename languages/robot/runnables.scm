; Run buttons. The test name is exposed to tasks as $ZED_CUSTOM_robot_test_name;
; see tasks.json for the tasks these buttons start.

; Each test case / task.
(test_cases_section
  (test_case_definition
    (name) @run @robot_test_name)
  (#set! tag robot-test))

; The whole suite, next to the *** Test Cases *** / *** Tasks *** header.
(test_cases_section
  (section_header) @run
  (#set! tag robot-suite))

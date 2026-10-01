; Adds a run button next to every test case / task. The test name is exposed
; to tasks as $ZED_CUSTOM_robot_test_name (see README for a tasks.json example).
(test_cases_section
  (test_case_definition
    (name) @run @robot_test_name)
  (#set! tag robot-test))

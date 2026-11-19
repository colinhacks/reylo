select n from docs where json_contains_path('{"a": 1}', 'one', '$.a', concat('$', left(n, 0)))

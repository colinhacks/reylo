select n from docs where json_contains_path('{"a": 1}', 'one', concat('$', left(n, 0)), '$.a')

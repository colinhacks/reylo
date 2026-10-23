select json_object('n', j.n) as o from json_table(:d::json('{"type": "array", "items": {"type": "string"}}'), '$[*]' columns (n text path '$')) as j

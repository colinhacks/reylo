select u.name from users u join json_table(:ids::json('{"type": "array", "items": {"type": "integer"}}'), '$[*]' columns (id int path '$')) as j on u.id = j.id

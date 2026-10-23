select u.id from users u join json_table(:codes::json('{"type": "array", "items": {"type": "string"}}'), '$[*]' columns (code varchar(20) path '$')) as j on u.code = j.code

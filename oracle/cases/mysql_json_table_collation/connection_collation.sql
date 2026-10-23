select u.id from users u join json_table(:names::json('{"type": "array", "items": {"type": "string"}}'), '$[*]' columns (legacy varchar(20) path '$')) as j on u.legacy = j.legacy

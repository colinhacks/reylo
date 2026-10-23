select id from users join json_table(:names::json('{"type": "array", "items": {"type": "string"}}'), '$[*]' columns (name varchar(40) character set utf8mb4 path '$')) as j using (name)

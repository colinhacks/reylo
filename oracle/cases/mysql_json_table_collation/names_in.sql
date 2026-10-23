select id, name from users where name in (select j.name from json_table(:names::json('{"type": "array", "items": {"type": "string"}}'), '$[*]' columns (name varchar(40) path '$')) as j)

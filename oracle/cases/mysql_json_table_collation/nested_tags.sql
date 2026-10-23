select u.id from users u join json_table(:doc::json('{"type": "object"}'), '$' columns (nested path '$.tags[*]' columns (tag varchar(20) path '$'))) as t on u.tag = t.tag

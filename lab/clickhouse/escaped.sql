-- ClickHouse reads a backslash in a string as an escape, so `:not` and `{nobody:String}` are inside the string, and `?` with `:` is its conditional, so `:early` is no parameter.
SELECT id, 'it\'s :not {nobody:String}' AS said, id > 1 ? 'late' :'early' AS era
FROM users
WHERE id = {id:UInt32} -- and name = {nobody:String}
LIMIT 1

SELECT id, name
FROM users
WHERE role IN {roles:Array(Enum8('admin' = 1, 'member' = 2))}
ORDER BY id

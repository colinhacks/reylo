SELECT id, name, email, role, created_at
FROM users
WHERE id = {id:UInt32}
LIMIT 1

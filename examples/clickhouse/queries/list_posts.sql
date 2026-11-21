SELECT p.id, p.title, p.body, u.name AS author
FROM posts p
JOIN users u ON u.id = p.user_id
WHERE u.role = {role:Enum8('admin' = 1, 'member' = 2)}
ORDER BY p.created_at, p.id

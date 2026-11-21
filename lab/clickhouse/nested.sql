SELECT u.id,
  groupArray(CAST((p.id, p.title, p.tag), 'Tuple(id UInt32, title String, tag Nullable(String))')) AS posts,
  groupArray((p.id, p.title)) AS pairs,
  count() AS n_posts,
  max(p.id) AS last_post
FROM users u
JOIN posts p ON p.user_id = u.id
WHERE u.id = {id:UInt32}
GROUP BY u.id
LIMIT 1

SELECT count() AS n, max(at) AS latest
FROM users
WHERE name LIKE {pattern:String}

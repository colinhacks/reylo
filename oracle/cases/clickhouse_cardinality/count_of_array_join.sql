SELECT count() AS n, sum(length(t)) AS letters FROM users ARRAY JOIN tags AS t

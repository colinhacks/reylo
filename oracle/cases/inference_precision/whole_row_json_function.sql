select u.id, to_jsonb(g) as step from users u left join lateral generate_series(u.id, 1) as g(n) on true

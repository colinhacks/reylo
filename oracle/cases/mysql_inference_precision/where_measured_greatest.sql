select u.name, p.title, p.published_at from users u left join posts p on p.user_id = u.id where greatest(p.id, 0) > 0 and nullif(p.published_at, '2000-01-01') < '2100-01-01'

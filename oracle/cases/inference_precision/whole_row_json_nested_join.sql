select u.id, to_jsonb(p) as post, to_jsonb(a) as author from users u left join (posts p join users a on a.id = p.user_id) on p.published_at is null

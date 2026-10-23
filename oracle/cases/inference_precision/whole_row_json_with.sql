with dated as (select p.user_id, p.title, p.published_at, p.title || '!' as loud from posts p) select u.id, to_jsonb(d) as post from users u left join dated d on d.title = u.name

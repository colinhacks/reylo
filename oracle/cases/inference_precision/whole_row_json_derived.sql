select u.id, to_jsonb(d) as post from users u left join (select p.id, p.title, p.published_at, case when p.id > 1 then p.title end as late from posts p) d on d.id = u.id

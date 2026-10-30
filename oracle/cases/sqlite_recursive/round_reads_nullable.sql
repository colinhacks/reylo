with recursive t (id, body) as (select id, title from posts union all select p.id, p.body from posts p join t on p.user_id = t.id limit 1000) select id, body from t

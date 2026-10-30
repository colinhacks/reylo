with recursive t (id, depth) as (select id, 0 from users union all select p.id, t.depth + 1 from posts p join t on p.user_id = t.id limit 1000) select id, depth from t

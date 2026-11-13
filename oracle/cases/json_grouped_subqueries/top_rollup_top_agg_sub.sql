select u.id, jsonb_agg((select jsonb_build_object('k', u.id) from posts p order by p.id limit 1)) as j from users u group by rollup (u.id)

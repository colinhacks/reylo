select (select jsonb_build_object('a', jsonb_agg((select max(q.n) from posts q where q.user_id = p.user_id))) from posts p group by p.user_id order by 1 limit 1) as j

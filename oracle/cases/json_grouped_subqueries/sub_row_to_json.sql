select (select jsonb_build_object('a', jsonb_agg(p), 'r', array_to_json(array_agg(p))) from posts p group by p.user_id order by 1 limit 1) as j

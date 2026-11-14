select (select jsonb_build_object('s', generate_series(1, 2), 't', generate_series(1, 3), 'c', count(*)) from posts p group by p.user_id order by 1 offset 2 limit 1) as j

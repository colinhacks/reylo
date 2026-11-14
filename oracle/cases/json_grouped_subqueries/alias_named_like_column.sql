select (select jsonb_build_object('m', max(name), 'a', jsonb_agg(name)) from posts name group by name.user_id order by 1 limit 1) as j from users u

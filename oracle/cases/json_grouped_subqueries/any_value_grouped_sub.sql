select (select jsonb_build_object('k', any_value(p.title), 'a', jsonb_agg(p.id), 'm', max(p.tag)) from posts p group by p.user_id order by 1 limit 1) as j

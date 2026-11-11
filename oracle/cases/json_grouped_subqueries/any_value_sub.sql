select (select jsonb_build_object('k', any_value(p.title), 'a', jsonb_agg(p.id)) from posts p where false) as j

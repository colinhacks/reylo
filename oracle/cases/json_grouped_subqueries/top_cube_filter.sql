select u.id, jsonb_build_object('a', jsonb_agg(u.name) filter (where u.id > 0), 'k', u.id) as j from users u group by cube (u.id)

select u.id, jsonb_build_object('w', jsonb_agg(u.id) over (), 'k', u.id) as j from users u group by rollup (u.id)

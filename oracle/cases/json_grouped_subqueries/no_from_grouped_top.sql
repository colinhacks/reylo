select u.id, (select jsonb_build_object('m', min(3), 's', sum(u.id), 'a', jsonb_agg(u.id))) as j from users u group by u.id

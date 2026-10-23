select users.id, jsonb_build_object('rows', (select coalesce(array_agg(q), '{}') from posts q where q.user_id = users.id)) as j from users

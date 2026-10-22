with s as (select user_id, jsonb_build_object('id', id) as j from posts) select coalesce(s.j, '[]') as j from users u left join s on s.user_id = u.id

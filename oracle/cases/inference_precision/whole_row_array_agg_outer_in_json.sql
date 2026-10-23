select u.id, jsonb_build_object('rows', array_agg(p)) as rows from users u left join posts p on p.user_id = u.id group by u.id

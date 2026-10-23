select u.id, jsonb_agg(p.*) as posts from users u left join posts p on p.user_id = u.id group by u.id

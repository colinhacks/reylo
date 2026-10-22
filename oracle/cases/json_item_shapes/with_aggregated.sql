with s as (select user_id, jsonb_build_object('id', id, 'title', title) as post from posts) select u.id, jsonb_agg(s.post) as posts from users u join s on s.user_id = u.id group by u.id

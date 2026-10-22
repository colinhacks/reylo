with ps as (select user_id, jsonb_agg(jsonb_build_object('id', id, 'title', title)) as posts from posts group by user_id) select u.name, ps.posts from users u left join ps on ps.user_id = u.id

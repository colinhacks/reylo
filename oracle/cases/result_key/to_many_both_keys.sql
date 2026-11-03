select u.id as user_id, p.id as post_id, t.id as tag_id from users u join posts p on p.author_id = u.id cross join tags t

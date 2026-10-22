select u.name from users u where u.id in (select user_id from posts union select 1)

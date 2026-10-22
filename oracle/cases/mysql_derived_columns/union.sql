select d.id from (select id from users union select user_id from posts) d

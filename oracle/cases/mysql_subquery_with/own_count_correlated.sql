select u.id, (with r as (select id, user_id from posts) select count(id) from r where r.user_id = u.id) as n from users u

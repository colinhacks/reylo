select u.id, d.user_id, d.n from users u left join (select user_id, (select count(*) from posts) as n from posts) d on d.user_id = u.id

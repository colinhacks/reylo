select u.id, d.n from users u left join (posts p join (select id, (select count(*) from posts) as n from users) d on d.id = p.user_id) on p.user_id = u.id

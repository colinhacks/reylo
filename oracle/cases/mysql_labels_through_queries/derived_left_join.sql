select p.id, x.r from posts p left join (select id, role as r from users) x on x.id = p.author_id + 10

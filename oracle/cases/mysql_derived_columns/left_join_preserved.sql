select d.id, d.n, p.title from (select id, (select count(*) from posts) as n from users) d left join posts p on p.user_id = d.id

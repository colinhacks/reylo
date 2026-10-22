select d.id, d.n, u.name from (select id, (select count(*) from posts) as n from users) d right join users u on u.id = d.id + 1000

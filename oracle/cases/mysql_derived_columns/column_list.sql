select d.a, d.b from (select id, (select count(*) from posts) as n from users) d (a, b)

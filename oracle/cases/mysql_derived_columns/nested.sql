select d.id, d.n from (select e.id, e.n from (select id, (select count(*) from posts) as n from users) e) d

select d.id, d.n from (select id, (select count(*) from posts) as n from users order by id limit 3) d

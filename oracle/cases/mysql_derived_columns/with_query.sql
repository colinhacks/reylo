with d as (select id, (select count(*) from posts) as n from users) select d.id, d.n from d

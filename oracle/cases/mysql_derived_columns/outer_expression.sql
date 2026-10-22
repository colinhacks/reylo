select d.n + 1 as m, concat(d.name, d.n) as label from (select name, (select count(*) from posts) as n from users) d

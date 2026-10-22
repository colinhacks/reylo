select d.n from (select (select count(*) from posts) as n from users union all select (select count(*) from users) from posts) d

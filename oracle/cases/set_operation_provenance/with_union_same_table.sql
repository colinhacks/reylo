with a as (select p.title as t from posts p union all select u.email from users u) select p.title, a.t from posts p join a on true

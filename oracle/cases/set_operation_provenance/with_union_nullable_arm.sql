with a as (select p.title as t from posts p union all select u.email from users u) select t from a

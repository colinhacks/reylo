with a as (select 1 as x union select 2) select u.* from users u, a

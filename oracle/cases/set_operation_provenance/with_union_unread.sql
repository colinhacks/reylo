with a as (select 1 as x union select 2) select u.name, u.email from users u where exists (select 1 from a)

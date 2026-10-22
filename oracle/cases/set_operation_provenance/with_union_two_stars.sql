with a as (select p.title as t, p.title as t2 from posts p union all select u.email, u.email from users u), e as (select) select a.*, u.name, e.* from a, users u, e

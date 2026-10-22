with a as (select p.id, p.title as text from posts p union all select c.id, c.body from comments c) select id, text from a

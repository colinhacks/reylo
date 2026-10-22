with o as (select id from users) select (with r as (select id from posts) select count(r.id) from r, o) as n

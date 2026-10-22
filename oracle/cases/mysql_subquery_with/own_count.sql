select (with r as (select id from posts) select count(id) from r) as n

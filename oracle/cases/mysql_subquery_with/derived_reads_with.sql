select (with r as (select id from posts) select count(id) from (select id from r) d) as n

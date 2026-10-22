select (select count(id) from (with r as (select id from posts) select id from r) d) as n

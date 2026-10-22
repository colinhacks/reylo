select (with r as (select id from posts) select max(id) from r) as last

select (with posts as (select id from users) select count(id) from posts) as n

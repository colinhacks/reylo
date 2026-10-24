with a as (select id, title from posts), b as (select * from a) select * from b where b.id = :id

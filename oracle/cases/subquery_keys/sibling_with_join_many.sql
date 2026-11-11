with a as (select id, title from posts), b as (select a.id, t.tag from a join tags t on t.post_id = a.id) select * from b where b.id = :id

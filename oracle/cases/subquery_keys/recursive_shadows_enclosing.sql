with a as (select id from posts) select * from (with recursive a (id) as (select author_id from posts union all select id from a where false) select * from (select id from a) s) t where t.id = :id

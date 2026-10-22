with a as (select id, to_jsonb(p) as doc from posts p), b as (select id, doc from a) select b.doc from b

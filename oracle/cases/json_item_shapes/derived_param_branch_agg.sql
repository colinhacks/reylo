select json_agg(x.o) as o from (select case when id > 1 then jsonb_build_object('id', id) else :fallback::doc_value end as o from posts) x

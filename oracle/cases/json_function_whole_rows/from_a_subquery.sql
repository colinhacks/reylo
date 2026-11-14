select (select to_jsonb(c.*) from posts p limit 1) as doc from bodies_for(1) c

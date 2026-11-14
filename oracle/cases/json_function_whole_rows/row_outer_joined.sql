select to_jsonb(c.*) as doc from posts p left join two_columns(p.id) c on true

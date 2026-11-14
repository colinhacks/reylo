select to_jsonb(c.*) as doc from posts p left join bodies_for(p.id + 100) c on true

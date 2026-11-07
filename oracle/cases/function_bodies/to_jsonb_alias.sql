select to_jsonb(c) as doc from comments_for(:p::int8) c

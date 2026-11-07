select to_jsonb(c.*) as doc from row_col(:p::int8) c

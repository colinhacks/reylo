select to_jsonb(x.*) as doc from unnest(array['a', 'b']) x

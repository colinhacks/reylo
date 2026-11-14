select jsonb_agg(c.*) as doc from bodies_for(:p::int8) c

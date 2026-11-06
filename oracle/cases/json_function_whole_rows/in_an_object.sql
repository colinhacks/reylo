select jsonb_build_object('a', c.*) as doc from bodies_for(:p::int8) c

select x.b from member m cross join lateral (select jsonb_build_object('n', m.first_name) as a, jsonb_build_object('k', 1) as b) x (b, a)

select x.o from member m cross join lateral (with t as (select 1 as one) select jsonb_build_object('n', m.first_name, 'one', t.one) as o from t) x

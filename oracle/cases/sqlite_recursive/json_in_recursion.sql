with recursive t (n, x, j) as (select 1, 'a', json_object('x', 'a') union all select n + 1, null, json_object('x', t.x) from t where n < 3) select n, x, j from t

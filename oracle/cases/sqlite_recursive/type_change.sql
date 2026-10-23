with recursive t (n, v) as (select 1, 1 union all select n + 1, 'text' from t where n < 3) select n, v from t

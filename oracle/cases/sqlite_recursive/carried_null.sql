with recursive t (n, x) as (select 1, null union all select n + 1, x from t where n < 3) select n, x from t

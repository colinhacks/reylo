with recursive t (n, x) as (select 1, 'a' union all select n + 1, null from t where n < 3) select n, x from t

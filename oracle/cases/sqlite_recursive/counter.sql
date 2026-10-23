with recursive t (n) as (select 1 union all select n + 1 from t limit 5) select n from t

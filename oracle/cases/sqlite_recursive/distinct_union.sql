with recursive t (n) as (select 1 union select n + 1 from t where n < 3) select n from t

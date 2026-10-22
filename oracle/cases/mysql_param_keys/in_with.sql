with t as (select name, count(*) as n from users group by name, :g) select name, n from t

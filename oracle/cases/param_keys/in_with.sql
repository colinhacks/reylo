with t as (select name, count(*)::int as n from users group by name, :g::text) select name, n from t

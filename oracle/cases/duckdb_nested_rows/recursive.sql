with recursive t(n, cs) as (select 1, array(select c from (select 1 as id) c) union all select n + 1, array(select c from (select null::integer as id) c) from t where n < 2) select cs from t

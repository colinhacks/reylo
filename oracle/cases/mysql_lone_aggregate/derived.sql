select d.b from (select b, count(*) as n from t where id = 1 and b is not null) d

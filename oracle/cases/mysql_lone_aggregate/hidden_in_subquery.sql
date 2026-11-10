select any_value(a) as v, (select count(*) from o where o.id = max(t.id)) as n from t where id = 99

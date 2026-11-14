select t.id, t.s, (select count(*) from o where o.t_id = t.id) as n from t where t.id = :id

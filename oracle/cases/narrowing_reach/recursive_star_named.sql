with recursive t as (select s.* from staff s where s.id = 1 union all select s.* from staff s join t on s.manager_id = t.id) select t.id, t.name from t

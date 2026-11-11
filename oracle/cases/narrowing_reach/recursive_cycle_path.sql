with recursive t as (select id, name from staff where id = 1 union all select s.id, s.name from staff s join t on s.manager_id = t.id) cycle id, name set is_cycle using path select t.path from t

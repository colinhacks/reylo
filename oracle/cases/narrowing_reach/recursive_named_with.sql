with recursive t(a, b) as (select id, name from staff where id = 1 union all select s.id, s.name from staff s join t on s.manager_id = t.a) search depth first by a set seq select t.seq from t

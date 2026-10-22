with recursive t (n, parent) as (select id, parent_id from categories where parent_id is null union all select c.id, c.parent_id from categories c join t on c.parent_id = t.n) select n, parent from t

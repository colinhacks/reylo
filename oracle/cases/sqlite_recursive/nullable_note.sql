with recursive t (id, note) as (select id, note from categories where parent_id is null union all select c.id, t.note from categories c join t on c.parent_id = t.id) select id, note from t

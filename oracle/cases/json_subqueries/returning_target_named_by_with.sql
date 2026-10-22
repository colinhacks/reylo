with users as (select :n::int as name) update users set p = 'x' where id = 1 returning jsonb_build_object('n', name) as j

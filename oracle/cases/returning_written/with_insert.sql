with i as (insert into t (id, note) values (10, 'x') returning note) select note from i

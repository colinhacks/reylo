with u as (update t set note = 'x' where id = 1 returning note) select note from u

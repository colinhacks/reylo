with u as (update trig set note = 'x' returning note) select note from u

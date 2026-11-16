insert into u (id, note, other) values (1, 'x', 'y') on conflict (id) do update set note = excluded.other returning note

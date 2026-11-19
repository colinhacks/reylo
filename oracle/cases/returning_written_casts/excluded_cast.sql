insert into u (id, note, other) values (1, 'x', row(1)::boxed) on conflict (id) do update set note = excluded.other returning note

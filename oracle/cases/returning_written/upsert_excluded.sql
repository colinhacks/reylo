insert into t (id, note) values (1, 'x'), (5, 'y') on conflict (id) do update set note = excluded.note returning note

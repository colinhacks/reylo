insert into trig (id, note) values (1, 'x') on conflict (id) do update set note = excluded.note returning note

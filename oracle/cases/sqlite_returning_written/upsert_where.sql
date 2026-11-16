insert into t (id, note) values (1, 'x'), (2, 'y') on conflict (id) do update set note = 'z' where n is not null returning note

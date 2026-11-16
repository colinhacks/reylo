insert into t (id, note) values (10, 'x'), (11, :note::text) returning note

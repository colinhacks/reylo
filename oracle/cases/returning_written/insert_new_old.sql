insert into t (id, note) values (10, 'x') returning new.note as after, old.note as before

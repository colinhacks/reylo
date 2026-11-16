insert into t (id, note) values (10, 'x') returning upper(note) || '!' as loud

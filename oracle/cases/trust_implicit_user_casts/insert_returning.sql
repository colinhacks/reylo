insert into t (id, s, v) values (5, 's', replace(row(1)::boxed, 'a', 'b')) returning v

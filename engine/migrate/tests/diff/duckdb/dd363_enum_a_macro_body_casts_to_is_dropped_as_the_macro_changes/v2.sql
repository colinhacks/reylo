create macro f(x) as x <> 'b'; create table t (id integer primary key, m varchar check (f(m)));

create macro valid(s) as length(s) > 0; create table u (id integer primary key, label varchar check (valid(label)), n integer);

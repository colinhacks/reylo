create macro valid(s) as length(s) > 0; create table p (id integer primary key, code varchar check (valid(code)));

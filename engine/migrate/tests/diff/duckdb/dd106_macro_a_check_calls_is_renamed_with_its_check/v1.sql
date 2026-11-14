create macro valid(c) as c <> ''; create table t (id integer primary key, code varchar check (valid(code)));

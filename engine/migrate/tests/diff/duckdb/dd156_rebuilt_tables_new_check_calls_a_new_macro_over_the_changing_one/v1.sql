create macro valid(s) as s <> ''; create table t (id integer primary key, code varchar check (valid(code)));

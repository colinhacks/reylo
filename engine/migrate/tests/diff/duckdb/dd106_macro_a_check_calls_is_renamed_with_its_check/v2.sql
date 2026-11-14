create macro ok_code(c) as c <> ''; create table t (id integer primary key, code varchar check (ok_code(code)));

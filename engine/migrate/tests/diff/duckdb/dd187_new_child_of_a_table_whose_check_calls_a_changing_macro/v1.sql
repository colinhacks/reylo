create macro valid(s) as s <> ''; create table parent (id integer primary key, code varchar check (valid(code)));

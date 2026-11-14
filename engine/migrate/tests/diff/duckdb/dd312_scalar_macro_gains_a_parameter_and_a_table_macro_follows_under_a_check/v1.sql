create macro valid(s) as s <> ''; create macro tm() as table select valid('x') as ok; create table t (id integer primary key, code varchar check (valid(code)));

create macro valid(s) as length(s) > 0; create macro zhelper(s) as valid(s) and s <> 'x'; create table t (id integer primary key, code varchar check (zhelper(code)));

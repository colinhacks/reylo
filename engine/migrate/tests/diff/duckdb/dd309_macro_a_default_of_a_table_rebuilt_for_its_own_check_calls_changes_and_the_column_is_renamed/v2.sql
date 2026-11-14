create macro dflt() as 'y'; create table t (id integer primary key, label varchar default dflt(), n integer check (n > 0));

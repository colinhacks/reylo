create macro dflt() as 'x'; create table t (id integer primary key, s varchar default dflt(), n integer);

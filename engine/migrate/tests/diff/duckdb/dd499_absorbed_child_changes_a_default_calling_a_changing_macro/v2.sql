create macro m(x) as x + 2; create table p (id integer primary key, a integer, check (a > 0)); create table c (id integer primary key, pid integer references p (id), d integer default m(5));

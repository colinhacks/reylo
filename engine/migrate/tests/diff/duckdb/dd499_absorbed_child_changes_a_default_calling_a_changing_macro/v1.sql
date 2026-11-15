create macro m(x) as x + 1; create table p (id integer primary key, a integer); create table c (id integer primary key, pid integer references p (id), d integer default m(1));

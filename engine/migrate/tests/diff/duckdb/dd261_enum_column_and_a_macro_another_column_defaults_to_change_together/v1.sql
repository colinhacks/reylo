create type mood as enum ('a', 'b'); create macro dflt() as 'x'; create table t (id integer primary key, m mood, n varchar default dflt());

create type mood as enum ('a', 'b', 'c'); create macro dflt() as 'z'; create table t (id integer primary key, m mood, n varchar default dflt());

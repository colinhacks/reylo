create table b (id integer primary key);
create table a (id integer primary key, b_id integer not null references b (id));

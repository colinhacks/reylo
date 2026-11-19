create table b (id text primary key);
create table a (id integer primary key, b_id text not null references b (id));

create table readings (id integer primary key, a integer not null, b integer, c integer);
alter table readings add constraint readings_c_not_null not null c not valid;

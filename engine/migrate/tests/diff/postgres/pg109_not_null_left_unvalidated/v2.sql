create table readings (id integer primary key, a integer, b integer, c integer);
alter table readings add constraint readings_a_not_null not null a not valid;
alter table readings add constraint readings_b_not_null not null b not valid;
create table fresh (id integer primary key, d integer);
alter table fresh add constraint fresh_d_not_null not null d not valid;

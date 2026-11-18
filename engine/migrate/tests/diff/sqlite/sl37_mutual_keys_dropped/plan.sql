-- loses data: a.b_email
-- loses data: a.name
-- loses data: b.a_name
-- loses data: b.email
drop index a_name_key;
drop index b_email_key;
create table new_a (id integer primary key autoincrement);
insert into new_a (id) select id from a;
drop table a;
alter table new_a rename to a;
create table new_b (id integer primary key autoincrement);
insert into new_b (id) select id from b;
drop table b;
alter table new_b rename to b;

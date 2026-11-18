-- name is not null with no default, so the copy fails on a table that has rows: give it a default, or add it nullable, fill it, then make it not null
-- b_email is not null with no default, so the copy fails on a table that has rows: give it a default, or add it nullable, fill it, then make it not null
create table new_a (id integer not null, name text not null, b_email text not null, constraint a_b_email_fkey foreign key (b_email) references b (email));
insert into new_a (id) select id from a;
drop table a;
alter table new_a rename to a;
-- email is not null with no default, so the copy fails on a table that has rows: give it a default, or add it nullable, fill it, then make it not null
-- a_name is not null with no default, so the copy fails on a table that has rows: give it a default, or add it nullable, fill it, then make it not null
create table new_b (id integer not null, email text not null, a_name text not null, constraint b_a_name_fkey foreign key (a_name) references a (name));
insert into new_b (id) select id from b;
drop table b;
alter table new_b rename to b;
create unique index a_name_key on a (name);
create unique index b_email_key on b (email);

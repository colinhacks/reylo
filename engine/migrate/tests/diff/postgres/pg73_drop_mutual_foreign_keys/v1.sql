create table kept (id integer primary key);
create table a (id bigserial primary key, name text not null);
create table b (id bigserial primary key, name text not null, a_id bigint references a (id));
alter table a add column b_id bigint;
alter table a add constraint a_b_fkey foreign key (b_id) references b (id);

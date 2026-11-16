create table users (id integer primary key, full_name text not null, n bigint not null, code text);
create view user_n as select id, n from users;

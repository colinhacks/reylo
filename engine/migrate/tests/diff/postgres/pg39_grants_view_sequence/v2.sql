create table users (id integer primary key, name text not null);
create sequence ticket_numbers;
create view user_names as select id, name from users;
grant select, insert on user_names to reader;
grant select, usage on sequence ticket_numbers to reader;

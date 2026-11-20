create table kept (id int);
create table users ();
create procedure "Add User"(name text) language sql as $$ insert into users default values $$;
drop table users;

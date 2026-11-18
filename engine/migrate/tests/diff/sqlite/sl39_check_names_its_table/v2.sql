create table users (id integer primary key autoincrement, age integer, name text, constraint some_check check("users"."age" > 21));
create index users_age on users (age);

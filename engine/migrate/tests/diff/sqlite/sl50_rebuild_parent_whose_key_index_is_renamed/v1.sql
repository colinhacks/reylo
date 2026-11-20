create table users (id integer primary key, email text not null, name text);
create unique index users_email on users (email);
create table logins (id integer primary key, email text references users (email));

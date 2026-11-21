create table users (id integer primary key, email text not null, name text check (length(name) > 0));
create unique index users_email_v2 on users (email);
create table logins (id integer primary key, email text references users (email));

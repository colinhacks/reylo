create table users (id integer primary key, email text collate nocase, name text);
create unique index users_email on users (email collate nocase);
create table logins (id integer primary key, email text references users (email));

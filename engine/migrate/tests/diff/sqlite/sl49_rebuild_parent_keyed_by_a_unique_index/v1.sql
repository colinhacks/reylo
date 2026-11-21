create table users (id integer primary key, email text not null, name text);
create unique index users_email on users (email);
create table logins (id integer primary key, email text references users (email) on delete cascade);
create unique index logins_email on logins (email);
create table sessions (id integer primary key, email text references logins (email));

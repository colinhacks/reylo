create table users (id integer primary key, name text not null);
comment on table users is 'The people who sign in';
comment on column users.name is 'Their full name';

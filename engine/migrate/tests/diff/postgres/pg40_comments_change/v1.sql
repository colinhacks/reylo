create table users (id integer primary key, name text not null);
comment on table users is 'People';
comment on column users.name is 'Their name';
comment on column users.id is 'Key';

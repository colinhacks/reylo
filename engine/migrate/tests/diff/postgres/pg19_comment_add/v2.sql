create table users (id integer primary key, full_name text not null, n integer not null, code text);
comment on table users is 'People';
comment on column users.n is 'A count';

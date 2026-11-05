-- With an extension that defines its own `=`, the equality a key is matched by has to be the built-in
-- for the types it compares, which a `with` query's cast names.
create extension vector;
create table users (id int primary key, account_id int not null, name text);
insert into users values (1, 1, 'a'), (2, 1, null);
create table notes (id int primary key, embedding vector(3) not null);
insert into notes values (1, '[1,0,0]');

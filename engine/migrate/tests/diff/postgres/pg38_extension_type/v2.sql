create extension citext;
create extension pgcrypto;
create table users (id integer primary key, email citext not null, token text default encode(gen_random_bytes(8), 'hex'));

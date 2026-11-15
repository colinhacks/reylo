create extension if not exists citext with schema public;
create extension if not exists pgcrypto with schema public;
alter table users alter column email type citext using email::citext;
alter table users add column token text default encode(gen_random_bytes(8), 'hex');

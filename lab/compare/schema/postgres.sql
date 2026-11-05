create extension if not exists pg_jsonschema;
create table users (id int primary key, email text not null unique, name text not null, nickname text, external_id uuid);
create table posts (id int primary key, author_id int not null references users(id), title text not null, score int, created_at timestamptz not null default now());
create table events (id int primary key, payload jsonb not null check (jsonb_matches_schema('{"type":"object","properties":{"x":{"type":"number"}},"required":["x"],"additionalProperties":false}', payload)));
create function add_one(n int) returns int language sql immutable as 'select n + 1';

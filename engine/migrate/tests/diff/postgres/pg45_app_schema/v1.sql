create extension if not exists pgcrypto;
create table profiles (
  id uuid primary key default gen_random_uuid(),
  name varchar(100) not null,
  created_at timestamptz not null default now()
);
create table posts (
  id bigint generated always as identity primary key,
  author uuid not null references profiles on delete cascade,
  title text not null,
  body text not null default ''
);

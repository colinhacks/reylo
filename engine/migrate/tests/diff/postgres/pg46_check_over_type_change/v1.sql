create table accounts (id integer primary key, tier varchar(10) not null check (tier in ('free', 'pro')));

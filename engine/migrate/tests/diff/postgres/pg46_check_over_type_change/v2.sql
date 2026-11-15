create table accounts (id integer primary key, tier varchar(20) not null check (tier in ('free', 'pro')));

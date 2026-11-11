-- A materialized view holds the rows of its last REFRESH: a key or a NOT NULL its table gained
-- since says nothing of them. It is typed as a stored relation, its columns nullable and its keys
-- its own unique indexes.
create table accounts (id int, email text, plan text, seats int);
insert into accounts values (1, 'a@example.com', 'free', null), (1, 'old@example.com', 'gone', null), (2, 'b@example.com', 'pro', 5);
create materialized view account_list as select id, email, plan, seats from accounts;
create materialized view account_index as select id, email, seats from accounts where plan <> 'gone';
create unique index account_index_id on account_index (id);
create view listed as select id, email, seats from account_list;
delete from accounts where plan = 'gone';
update accounts set seats = 0 where seats is null;
alter table accounts add primary key (id);
alter table accounts add constraint accounts_email_key unique (email);
alter table accounts alter column email set not null;
alter table accounts alter column seats set not null;
alter table accounts add constraint accounts_plan_check check (plan in ('free', 'pro'));

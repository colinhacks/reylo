create table accounts (id serial primary key, balance numeric not null);
create sequence ticket_seq;
create collation ci (provider = icu, locale = 'und-u-ks-level2', deterministic = false);
create publication audit_feed for table accounts;
create rule prevent_negative as on insert to accounts where new.balance < 0 do instead nothing;

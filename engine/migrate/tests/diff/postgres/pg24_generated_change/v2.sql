create table items (id integer primary key, price numeric not null, qty integer not null, total numeric generated always as (price * qty * 1.2) stored);

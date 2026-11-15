create table "Order Items" (id integer primary key, "Unit Price" real not null, qty integer);
create index "Order Items by price" on "Order Items" ("Unit Price");

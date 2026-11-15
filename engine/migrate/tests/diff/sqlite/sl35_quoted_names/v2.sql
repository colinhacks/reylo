create table "Order Items" (id integer primary key, "Unit Price" real not null, qty integer not null default 1, "Line Note" text);
create index "Order Items by price" on "Order Items" ("Unit Price");

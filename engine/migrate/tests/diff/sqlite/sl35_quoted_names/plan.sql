create table "new_Order Items" (id integer primary key, "Unit Price" real not null, qty integer not null default 1, "Line Note" text);
insert into "new_Order Items" (id, "Unit Price", qty) select id, "Unit Price", qty from "Order Items";
drop table "Order Items";
alter table "new_Order Items" rename to "Order Items";
create index "Order Items by price" on "Order Items" ("Unit Price");

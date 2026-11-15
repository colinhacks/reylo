drop index "Qty Index";
create temporary table reylo_copy_select as select * from "select";
create temporary table "reylo_copy_Order Items" as select * from "Order Items";
drop table "Order Items";
drop table "select";
create table "select" (Key integer, "from" varchar, primary key (Key), check (("from" != '')));
create table "Order Items" ("Item ID" integer, "Select ID" integer, "where" integer, primary key ("Item ID"), foreign key ("Select ID") references "select" (Key));
insert into "select" (Key, "from") select Key, "from" from temp.main.reylo_copy_select;
insert into "Order Items" ("Item ID", "Select ID", "where") select "Item ID", "Select ID", qty from temp.main."reylo_copy_Order Items";
drop table temp.main.reylo_copy_select;
drop table temp.main."reylo_copy_Order Items";
-- "Order Items".qty is renamed "where", keeping its rows. If they are two columns, drop the one and add the other instead.
create index "Qty Index" on "Order Items"("where");
comment on index "Qty Index" is 'by qty';

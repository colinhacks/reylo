create table "select" ("Key" integer primary key, "from" varchar);
     create table "Order Items" ("Item ID" integer primary key, "Select ID" integer references "select" ("Key"), qty integer);
     create index "Qty Index" on "Order Items" (qty);
     comment on index "Qty Index" is 'by qty';

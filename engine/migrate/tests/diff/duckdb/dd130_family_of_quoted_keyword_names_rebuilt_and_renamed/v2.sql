create table "select" ("Key" integer primary key, "from" varchar check ("from" <> ''));
     create table "Order Items" ("Item ID" integer primary key, "Select ID" integer references "select" ("Key"), "where" integer);
     create index "Qty Index" on "Order Items" ("where");
     comment on index "Qty Index" is 'by qty';

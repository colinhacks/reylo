create table t (id integer primary key, "Order" integer, "select" varchar, r struct("A" integer), n integer check (n > 0), check ("Order" > 0), check ("select" <> ''), check (r['A'] > 0));

-- Text cast to json or jsonb inside built JSON is JSON of any shape, never text, while a cast of
-- what the statement builds keeps its shape; and a parameter under two casts is a value of the
-- outer cast's type, which the describe does not report for the placeholder.
create table doc (id int primary key, j jsonb not null, n jsonb, label text not null);

-- Text cast to json or jsonb inside built JSON is JSON of any shape, never text, while a cast of
-- what the statement builds keeps its shape; and a parameter under two casts is a value of the
-- outer cast's type, which the describe does not report for the placeholder. A jsonb column holds
-- JSON's null however NOT NULL it is, and a document holds that as a null.
create table doc (id int primary key, j jsonb not null, n jsonb, label text not null);
-- A jsonb[] column a CHECK keeps free of SQL NULL still holds JSON's null as an element.
create table docs_arr (id int primary key, arr jsonb[] not null check (array_position(arr, null) is null));

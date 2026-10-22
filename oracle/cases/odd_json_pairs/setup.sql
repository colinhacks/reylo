-- A JSON object built from an odd count of arguments, which Postgres prepares and refuses when
-- it runs, and a statement that ends too soon.
create table t (id int primary key, name text not null);

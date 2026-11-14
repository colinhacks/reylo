create table users (id serial primary key, name text not null, email text unique);
create table one_column (x int not null);
create table no_columns ();

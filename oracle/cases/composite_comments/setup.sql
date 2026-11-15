-- A composite type's attribute comments, and a table's column comments, which are its row type's, are the comments of the struct's fields wherever the row is a value: a column, an array's element, JSON content.
create type address as (street text, city text);
comment on column address.street is 'The street and number';
comment on column address.city is 'As the post office
spells it';
create table people (id int primary key, name text not null, home address, past address[]);
comment on column people.name is 'Shown on the profile';
comment on column people.home is 'Where they live now';
insert into people values (1, 'a', row('1 Main', 'X'), array[row('2 Side', 'Y')::address]), (2, 'b', null, null);

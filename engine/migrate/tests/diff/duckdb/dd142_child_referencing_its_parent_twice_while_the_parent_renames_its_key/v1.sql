create table person (id integer primary key, name varchar); create table friendship (a integer references person (id), b integer references person (id), since date);

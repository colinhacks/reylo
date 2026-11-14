create table person (pid integer primary key, name varchar check (name <> '')); create table friendship (a integer references person (pid), b integer references person (pid), since date);

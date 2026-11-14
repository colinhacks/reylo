create table t (id integer, "primary" varchar, "unique" varchar, note varchar check (note <> 'primary key, unique'), primary key (id, "primary"));

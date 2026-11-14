create table people (id integer primary key, name text not null check (name <> ''), boss_id integer references people (id));

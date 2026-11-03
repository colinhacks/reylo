create table people (id integer primary key, name text not null, boss_id integer references people (id));

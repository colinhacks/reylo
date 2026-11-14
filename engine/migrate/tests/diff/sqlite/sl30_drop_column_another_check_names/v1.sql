create table ranges (id integer primary key, lo integer not null, hi integer not null, check (lo <= hi));

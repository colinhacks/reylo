create table p (id integer primary key, n integer); create table c (id integer, p_id integer references p (id), note varchar); create index c_note on c (note); comment on table c is 'kids';

create macro valid(c) as c <> '';
     create table p (id integer primary key, code varchar check (valid(code)));
     create table c (id integer primary key, p_id integer references p (id), note varchar);

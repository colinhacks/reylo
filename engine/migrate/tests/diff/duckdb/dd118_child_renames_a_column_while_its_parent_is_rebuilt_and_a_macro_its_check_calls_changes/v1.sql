create macro valid(c) as c <> '';
     create table p (id integer primary key, name varchar);
     create table c (id integer primary key, p_id integer references p (id), code varchar check (valid(code)), note varchar);

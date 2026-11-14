create macro gen() as 'g2';
     create table p (id integer primary key, name varchar check (name <> ''));
     create table c (id integer primary key, p_id integer references p (id), label varchar default gen());

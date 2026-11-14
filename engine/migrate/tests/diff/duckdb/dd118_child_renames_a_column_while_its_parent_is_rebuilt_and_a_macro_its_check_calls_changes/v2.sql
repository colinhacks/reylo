create macro valid(c) as length(c) > 0;
     create table p (id integer primary key, name varchar check (name <> ''));
     create table c (id integer primary key, p_id integer references p (id), code varchar check (valid(code)), remark varchar);

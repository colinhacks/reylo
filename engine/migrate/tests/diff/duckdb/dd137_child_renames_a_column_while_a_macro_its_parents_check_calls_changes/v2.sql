create macro valid(c) as length(c) > 0;
     create table p (id integer primary key, code varchar check (valid(code)));
     create table c (id integer primary key, p_id integer references p (id), remark varchar);

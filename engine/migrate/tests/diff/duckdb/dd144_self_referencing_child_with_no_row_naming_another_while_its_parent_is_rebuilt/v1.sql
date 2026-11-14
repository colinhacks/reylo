create table p (id integer primary key, name varchar); create table node (id integer primary key, p_id integer references p (id), up integer references node (id));

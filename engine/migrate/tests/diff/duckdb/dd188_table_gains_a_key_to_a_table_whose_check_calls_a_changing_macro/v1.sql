create macro valid(s) as s <> ''; create table parent (id integer primary key, code varchar check (valid(code))); create table child (id integer primary key, parent_id integer);
